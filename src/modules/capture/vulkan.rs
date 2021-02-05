//! Color conversion and pixel exporting using Vulkan.
//!
//! # No Sampling
//!
//! 1. Transition image_frame to OpenGL, signalling a semaphore, and transition image_sample to
//!    TRANSFER_DST_OPTIMAL.
//! 2. Loop:
//!     1) Acquire image_frame.
//!     2) Blit image_frame to image_sample.
//!     3) Transition image_frame to OpenGL.
//!
//!     4) Transition image_sample:
//!         TRANSFER_WRITE => SHADER_READ
//!         TRANSFER_DST_OPTIMAL => SHADER_READ_ONLY_OPTIMAL
//!         TRANSFER => COMPUTE_SHADER
//!     5) Dispatch compute shader.
//!     6) Transition image_sample:
//!         SHADER_READ => TRANSFER_WRITE
//!         SHADER_READ_ONLY_OPTIMAL => TRANSFER_DST_OPTIMAL
//!         COMPUTE_SHADER => TRANSFER
//!
//! # Sampling
//!
//! On every new frame:
//! 1. Add it to image_sample.
//! 2. On remainder overflow:
//!     1. Convert colors.
//!     2. Output one or more frames.
//!     3. Clear image_sample.
//!     4. Add remainder of the frame to image_sample.
//!
//! 1. Transition image_frame to OpenGL, signalling a semaphore, and transition image_sample to
//!    GENERAL.
//! 2. Loop:
//!     - No remainder overflow:
//!         1) Acquire image_frame.
//!         2) Dispatch compute shader.
//!         3) Transition image_frame to OpenGL.
//!
//!         4) Barrier on image_sample:
//!             SHADER_WRITE => SHADER_WRITE
//!             GENERAL => GENERAL
//!             COMPUTE_SHADER => COMPUTE_SHADER
//!
//!     - Remainder overflow:
//!         1) Acquire image_frame.
//!         2) Dispatch compute shader.
//!         3) Transition image_frame to OpenGL.
//!
//!         4) Transition image_sample:
//!             SHADER_WRITE => SHADER_READ
//!             GENERAL => SHADER_READ_ONLY_OPTIMAL
//!             COMPUTE_SHADER => COMPUTE_SHADER
//!         5) Dispatch compute shader.
//!         6) Transition image_sample:
//!             SHADER_READ => TRANSFER_WRITE
//!             SHADER_READ_ONLY_OPTIMAL => TRANSFER_DST_OPTIMAL
//!             COMPUTE_SHADER => TRANSFER
//!         7) Clear image_sample.
//!         8) Transition image_sample:
//!             TRANSFER_DST => SHADER_WRITE
//!             TRANSFER_DST_OPTIMAL => GENERAL
//!             TRANSFER => COMPUTE_SHADER
//!         9) Dispatch compute shader.
//!         10) Barrier on image_sample:
//!             SHADER_WRITE => SHADER_WRITE
//!             GENERAL => GENERAL
//!             COMPUTE_SHADER => COMPUTE_SHADER

use std::{ffi::CStr, io::Cursor, mem, slice, str};

use ash::{
    util::read_spv,
    version::{DeviceV1_0, InstanceV1_0},
    vk,
};
use color_eyre::eyre::{self, ensure, eyre};
use rust_hawktracer::*;

use super::{muxer::Muxer, ExternalObject};

pub struct Vulkan {
    width: u32,
    height: u32,
    queue_family_index: u32,
    device: ash::Device,
    command_pool: vk::CommandPool,
    command_buffer_sampling: vk::CommandBuffer,
    command_buffer_color_conversion: vk::CommandBuffer,
    queue: vk::Queue,
    image_frame: vk::Image,
    image_frame_memory: vk::DeviceMemory,
    image_frame_memory_size: u64,
    #[cfg(unix)]
    external_memory_fd: ash::extensions::khr::ExternalMemoryFd,
    #[cfg(windows)]
    external_memory_win32_fn: vk::KhrExternalMemoryWin32Fn,
    image_sample: vk::Image,
    image_sample_memory: vk::DeviceMemory,
    semaphore: vk::Semaphore,
    #[cfg(unix)]
    external_semaphore_fd_fn: vk::KhrExternalSemaphoreFdFn,
    #[cfg(windows)]
    external_semaphore_win32_fn: vk::KhrExternalSemaphoreWin32Fn,
    buffer: vk::Buffer,
    buffer_memory: vk::DeviceMemory,
    buffer_color_conversion_output: vk::Buffer,
    buffer_color_conversion_output_memory: vk::DeviceMemory,
    sampler_sample: vk::Sampler,
    sampler_frame: vk::Sampler,
    image_view_sample: vk::ImageView,
    image_view_frame: vk::ImageView,
    descriptor_set_layout_color_conversion: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_color_conversion: vk::DescriptorSet,
    shader_module: vk::ShaderModule,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
}

#[derive(Debug)]
pub struct ExternalHandles {
    pub external_image_frame_memory: ExternalObject,
    pub external_semaphore: ExternalObject,
    pub size: u64,
}

impl Drop for Vulkan {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_pipeline(self.pipeline, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_shader_module(self.shader_module, None);
            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layout_color_conversion, None);
            self.device
                .free_memory(self.buffer_color_conversion_output_memory, None);
            self.device
                .destroy_buffer(self.buffer_color_conversion_output, None);
            self.device.free_memory(self.buffer_memory, None);
            self.device.destroy_buffer(self.buffer, None);
            self.device.destroy_semaphore(self.semaphore, None);
            self.device.destroy_image_view(self.image_view_sample, None);
            self.device.destroy_sampler(self.sampler_sample, None);
            self.device.free_memory(self.image_sample_memory, None);
            self.device.destroy_image(self.image_sample, None);
            self.device.destroy_image_view(self.image_view_frame, None);
            self.device.destroy_sampler(self.sampler_frame, None);
            self.device.free_memory(self.image_frame_memory, None);
            self.device.destroy_image(self.image_frame, None);
            self.device.free_command_buffers(
                self.command_pool,
                &[
                    self.command_buffer_sampling,
                    self.command_buffer_color_conversion,
                ],
            );
            self.device.destroy_command_pool(self.command_pool, None);
            self.device.destroy_device(None);
        }
    }
}

impl Vulkan {
    #[cfg(unix)]
    pub fn external_image_frame_memory(&self) -> eyre::Result<ExternalObject> {
        let create_info = vk::MemoryGetFdInfoKHR::builder()
            .memory(self.image_frame_memory)
            .handle_type(vk::ExternalMemoryHandleTypeFlags::EXTERNAL_MEMORY_HANDLE_TYPE_OPAQUE_FD);
        let fd = unsafe { self.external_memory_fd.get_memory_fd(&create_info)? };
        Ok(fd)
    }

    #[cfg(windows)]
    pub fn external_image_frame_memory(&self) -> eyre::Result<ExternalObject> {
        let create_info = vk::MemoryGetWin32HandleInfoKHR::builder()
            .memory(self.image_frame_memory)
            .handle_type(
                vk::ExternalMemoryHandleTypeFlags::EXTERNAL_MEMORY_HANDLE_TYPE_OPAQUE_WIN32,
            );
        let mut memory_handle = std::ptr::null_mut();
        let rv = unsafe {
            self.external_memory_win32_fn.get_memory_win32_handle_khr(
                self.device.handle(),
                &*create_info,
                &mut memory_handle,
            )
        };
        ensure!(
            rv == vk::Result::SUCCESS,
            "get_memory_win32_handle_khr() returned an error: {}",
            rv
        );
        Ok(memory_handle)
    }

    pub fn image_frame_memory_size(&self) -> u64 {
        self.image_frame_memory_size
    }

    pub fn external_handles(&self) -> eyre::Result<ExternalHandles> {
        let external_image_frame_memory = self.external_image_frame_memory()?;
        let external_semaphore = self.external_semaphore()?;
        let size = self.image_frame_memory_size();

        Ok(ExternalHandles {
            external_image_frame_memory,
            external_semaphore,
            size,
        })
    }

    #[cfg(unix)]
    pub fn external_semaphore(&self) -> eyre::Result<ExternalObject> {
        let create_info = vk::SemaphoreGetFdInfoKHR::builder()
            .semaphore(self.semaphore)
            .handle_type(
                vk::ExternalSemaphoreHandleTypeFlags::EXTERNAL_SEMAPHORE_HANDLE_TYPE_OPAQUE_FD,
            );
        let mut semaphore_fd = -1;
        let rv = unsafe {
            self.external_semaphore_fd_fn.get_semaphore_fd_khr(
                self.device.handle(),
                &*create_info,
                &mut semaphore_fd,
            )
        };
        ensure!(
            rv == vk::Result::SUCCESS,
            "get_semaphore_fd_khr() returned an error: {}",
            rv
        );
        Ok(semaphore_fd)
    }

    #[cfg(windows)]
    pub fn external_semaphore(&self) -> eyre::Result<ExternalObject> {
        let create_info = vk::SemaphoreGetWin32HandleInfoKHR::builder()
            .semaphore(self.semaphore)
            .handle_type(
                vk::ExternalSemaphoreHandleTypeFlags::EXTERNAL_SEMAPHORE_HANDLE_TYPE_OPAQUE_WIN32,
            );
        let mut semaphore_handle = std::ptr::null_mut();
        let rv = unsafe {
            self.external_semaphore_win32_fn
                .get_semaphore_win32_handle_khr(
                    self.device.handle(),
                    &*create_info,
                    &mut semaphore_handle,
                )
        };
        ensure!(
            rv == vk::Result::SUCCESS,
            "get_semaphore_win32_handle_khr() returned an error: {}",
            rv
        );
        Ok(semaphore_handle)
    }

    #[hawktracer(acquire_image)]
    pub unsafe fn acquire_image(&self) -> eyre::Result<()> {
        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        self.device
            .begin_command_buffer(self.command_buffer_sampling, &begin_info)?;

        // Acquire the image from OpenGL.
        let image_frame_memory_barrier = vk::ImageMemoryBarrier::builder()
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::empty())
            .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
            .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_EXTERNAL)
            .dst_queue_family_index(self.queue_family_index)
            .image(self.image_frame)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        self.device.cmd_pipeline_barrier(
            self.command_buffer_sampling,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[*image_frame_memory_barrier],
        );

        // Barrier for the sampling image must've been inserted by previous code.

        // Blit image_frame to image_sample.
        let image_blit = vk::ImageBlit::builder()
            .src_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                layer_count: 1,
                ..Default::default()
            })
            .src_offsets([
                vk::Offset3D::default(),
                vk::Offset3D {
                    x: self.width as i32,
                    y: self.height as i32,
                    z: 1,
                },
            ])
            .dst_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                layer_count: 1,
                ..Default::default()
            })
            .dst_offsets([
                vk::Offset3D::default(),
                vk::Offset3D {
                    x: self.width as i32,
                    y: self.height as i32,
                    z: 1,
                },
            ]);

        self.device.cmd_blit_image(
            self.command_buffer_sampling,
            self.image_frame,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            self.image_sample,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[*image_blit],
            vk::Filter::NEAREST,
        );

        // Transfer image_frame back to OpenGL.
        let image_frame_memory_barrier = vk::ImageMemoryBarrier::builder()
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::empty())
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::GENERAL)
            .src_queue_family_index(self.queue_family_index)
            .dst_queue_family_index(vk::QUEUE_FAMILY_EXTERNAL)
            .image(self.image_frame)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        self.device.cmd_pipeline_barrier(
            self.command_buffer_sampling,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[*image_frame_memory_barrier],
        );

        self.device
            .end_command_buffer(self.command_buffer_sampling)?;

        let semaphores = [self.semaphore];
        let command_buffers = [self.command_buffer_sampling];
        let submit_info = vk::SubmitInfo::builder()
            .wait_semaphores(&semaphores)
            .wait_dst_stage_mask(&[vk::PipelineStageFlags::ALL_COMMANDS])
            .signal_semaphores(&semaphores)
            .command_buffers(&command_buffers);
        self.device
            .queue_submit(self.queue, &[*submit_info], vk::Fence::null())?;

        Ok(())
    }

    #[hawktracer(convert_colors_and_mux)]
    pub unsafe fn convert_colors_and_mux(
        &self,
        muxer: &mut Muxer,
        frames: usize,
    ) -> eyre::Result<()> {
        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        self.device
            .begin_command_buffer(self.command_buffer_color_conversion, &begin_info)?;

        // Set a barrier for the color conversion stage.
        let image_sample_memory_barrier = vk::ImageMemoryBarrier::builder()
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ)
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(self.image_sample)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        self.device.cmd_pipeline_barrier(
            self.command_buffer_color_conversion,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::COMPUTE_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[*image_sample_memory_barrier],
        );

        // Run the color conversion shader.
        self.device.cmd_bind_pipeline(
            self.command_buffer_color_conversion,
            vk::PipelineBindPoint::COMPUTE,
            self.pipeline,
        );
        self.device.cmd_bind_descriptor_sets(
            self.command_buffer_color_conversion,
            vk::PipelineBindPoint::COMPUTE,
            self.pipeline_layout,
            0,
            &[self.descriptor_set_color_conversion],
            &[],
        );

        self.device.cmd_dispatch(
            self.command_buffer_color_conversion,
            (self.width + 4 - 1) / 4,
            (self.height + 4 - 1) / 4,
            1,
        );

        // Barrier for the pixel buffer to copy it to the host-visible buffer.
        let buffer_memory_barrier = vk::BufferMemoryBarrier::builder()
            .src_access_mask(vk::AccessFlags::SHADER_WRITE)
            .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .buffer(self.buffer_color_conversion_output)
            .offset(0)
            .size(vk::WHOLE_SIZE);

        self.device.cmd_pipeline_barrier(
            self.command_buffer_color_conversion,
            vk::PipelineStageFlags::COMPUTE_SHADER,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[*buffer_memory_barrier],
            &[],
        );

        let buffer_copy =
            vk::BufferCopy::builder().size(self.width as u64 * self.height as u64 / 2 * 3);
        self.device.cmd_copy_buffer(
            self.command_buffer_color_conversion,
            self.buffer_color_conversion_output,
            self.buffer,
            &[*buffer_copy],
        );

        // Barrier for the pixel buffer to read it from the host.
        let buffer_memory_barrier = vk::BufferMemoryBarrier::builder()
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::HOST_READ)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .buffer(self.buffer)
            .offset(0)
            .size(vk::WHOLE_SIZE);

        self.device.cmd_pipeline_barrier(
            self.command_buffer_color_conversion,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::HOST,
            vk::DependencyFlags::empty(),
            &[],
            &[*buffer_memory_barrier],
            &[],
        );

        // Barrier for the next frame capture.
        let image_sample_memory_barrier = vk::ImageMemoryBarrier::builder()
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(self.image_sample)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        self.device.cmd_pipeline_barrier(
            self.command_buffer_color_conversion,
            vk::PipelineStageFlags::COMPUTE_SHADER,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[*image_sample_memory_barrier],
        );

        self.device
            .end_command_buffer(self.command_buffer_color_conversion)?;

        let create_info = vk::FenceCreateInfo::default();
        let fence = self.device.create_fence(&create_info, None)?;

        let command_buffers = [self.command_buffer_color_conversion];
        let submit_info = vk::SubmitInfo::builder().command_buffers(&command_buffers);
        self.device
            .queue_submit(self.queue, &[*submit_info], fence)?;

        {
            scoped_tracepoint!(wait_for_fence_);

            self.device
                .wait_for_fences(&[fence], true, u64::max_value())?;
        }

        let pixels = self.device.map_memory(
            self.buffer_memory,
            0,
            vk::WHOLE_SIZE,
            vk::MemoryMapFlags::empty(),
        )?;

        let mapped_memory_range = vk::MappedMemoryRange::builder()
            .memory(self.buffer_memory)
            .size(vk::WHOLE_SIZE);
        self.device
            .invalidate_mapped_memory_ranges(&[*mapped_memory_range])?;

        // Save into a file.
        {
            let pixels: &[u8] = slice::from_raw_parts(
                pixels.cast(),
                self.width as usize * self.height as usize / 2 * 3,
            );

            for _ in 0..frames {
                muxer.write_video_frame(pixels)?;
            }
        }

        self.device.unmap_memory(self.buffer_memory);

        // Cleanup.
        self.device.destroy_fence(fence, None);

        Ok(())
    }
}

#[hawktracer(vulkan_init)]
pub fn init(width: u32, height: u32) -> eyre::Result<Vulkan> {
    // TODO: handle weird resolutions.
    ensure!(
        width % 2 == 0 && height % 2 == 0,
        "can't handle odd resulutions yet: {}×{}",
        width,
        height
    );

    let instance = crate::vulkan::VULKAN.get().unwrap().instance();

    // Physical device.
    let physical_devices = unsafe { instance.enumerate_physical_devices()? };
    let mut physical_device_index = 0;
    debug!("physical devices:");
    for (i, &device) in physical_devices.iter().enumerate() {
        let properties = unsafe { instance.get_physical_device_properties(device) };
        debug!("\t{}: [{:?}] {}", i, properties.device_type, unsafe {
            str::from_utf8_unchecked(CStr::from_ptr(properties.device_name.as_ptr()).to_bytes())
        });

        if properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
            physical_device_index = i;
        }
    }

    debug!("choosing physical device {}", physical_device_index);
    let physical_device = physical_devices[physical_device_index];

    // Memory properties.
    let memory_properties =
        unsafe { instance.get_physical_device_memory_properties(physical_device) };

    // Queue family index.
    let queue_family_properties =
        unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
    let queue_family_index = queue_family_properties
        .into_iter()
        .enumerate()
        .find(|(_, properties)| properties.queue_flags.contains(vk::QueueFlags::COMPUTE))
        .map(|(i, _)| i)
        .ok_or_else(|| eyre!("couldn't find a compute queue family"))?
        as u32;

    // Logical device.
    let queue_create_infos = [vk::DeviceQueueCreateInfo {
        queue_family_index,
        queue_count: 1,
        p_queue_priorities: &1.,
        ..Default::default()
    }];
    let extension_names = [
        #[cfg(unix)]
        ash::extensions::khr::ExternalMemoryFd::name().as_ptr(),
        #[cfg(windows)]
        vk::KhrExternalMemoryWin32Fn::name().as_ptr(),
        #[cfg(unix)]
        vk::KhrExternalSemaphoreFdFn::name().as_ptr(),
        #[cfg(windows)]
        vk::KhrExternalSemaphoreWin32Fn::name().as_ptr(),
        vk::Khr8bitStorageFn::name().as_ptr(),
    ];
    let mut physical_device_8_bit_storage_features =
        vk::PhysicalDevice8BitStorageFeatures::builder()
            .storage_buffer8_bit_access(true)
            .uniform_and_storage_buffer8_bit_access(true);
    let create_info = vk::DeviceCreateInfo::builder()
        .queue_create_infos(&queue_create_infos)
        .enabled_extension_names(&extension_names)
        .push_next(&mut physical_device_8_bit_storage_features);
    let device = unsafe { instance.create_device(physical_device, &create_info, None)? };

    // Command pool.
    let create_info = vk::CommandPoolCreateInfo::builder()
        .queue_family_index(queue_family_index)
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
    let command_pool = unsafe { device.create_command_pool(&create_info, None)? };

    // Command buffer.
    let create_info = vk::CommandBufferAllocateInfo::builder()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(2);
    let command_buffers = unsafe { device.allocate_command_buffers(&create_info)? };
    let command_buffer_sampling = command_buffers[0];
    let command_buffer_color_conversion = command_buffers[1];

    // Queue.
    let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

    // Image for the OpenGL frame.
    #[cfg(unix)]
    let mut external_memory_image_create_info = vk::ExternalMemoryImageCreateInfo::builder()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::EXTERNAL_MEMORY_HANDLE_TYPE_OPAQUE_FD);
    #[cfg(windows)]
    let mut external_memory_image_create_info = vk::ExternalMemoryImageCreateInfo::builder()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::EXTERNAL_MEMORY_HANDLE_TYPE_OPAQUE_WIN32);
    let create_info = vk::ImageCreateInfo::builder()
        .image_type(vk::ImageType::TYPE_2D)
        .format(vk::Format::R8G8B8A8_UNORM)
        .extent(vk::Extent3D {
            width,
            height,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        // Required for exporting to OpenGL.
        .usage(
            vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST
                | vk::ImageUsageFlags::SAMPLED
                | vk::ImageUsageFlags::STORAGE
                | vk::ImageUsageFlags::COLOR_ATTACHMENT
                | vk::ImageUsageFlags::INPUT_ATTACHMENT,
        )
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .push_next(&mut external_memory_image_create_info);
    let image_frame = unsafe { device.create_image(&create_info, None)? };

    let image_frame_memory_requirements =
        unsafe { device.get_image_memory_requirements(image_frame) };
    let image_frame_memory_type_index = find_memorytype_index(
        &image_frame_memory_requirements,
        &memory_properties,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .ok_or_else(|| eyre!("couldn't find image_frame memory type"))?;
    #[cfg(unix)]
    let mut export_memory_allocate_info = vk::ExportMemoryAllocateInfo::builder()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::EXTERNAL_MEMORY_HANDLE_TYPE_OPAQUE_FD);
    #[cfg(windows)]
    let mut export_memory_allocate_info = vk::ExportMemoryAllocateInfo::builder()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::EXTERNAL_MEMORY_HANDLE_TYPE_OPAQUE_WIN32);
    let mut memory_dedicated_allocate_info =
        vk::MemoryDedicatedAllocateInfo::builder().image(image_frame);
    let create_info = vk::MemoryAllocateInfo::builder()
        .allocation_size(image_frame_memory_requirements.size)
        .memory_type_index(image_frame_memory_type_index)
        .push_next(&mut export_memory_allocate_info)
        .push_next(&mut memory_dedicated_allocate_info);
    let image_frame_memory = unsafe { device.allocate_memory(&create_info, None)? };
    unsafe { device.bind_image_memory(image_frame, image_frame_memory, 0)? };

    // External memory.
    #[cfg(unix)]
    let external_memory_fd = ash::extensions::khr::ExternalMemoryFd::new(instance, &device);
    #[cfg(windows)]
    let external_memory_win32_fn = vk::KhrExternalMemoryWin32Fn::load(|name| unsafe {
        mem::transmute(instance.get_device_proc_addr(device.handle(), name.as_ptr()))
    });

    // Sampler for the image for the OpenGL frame.
    let create_info = vk::SamplerCreateInfo::builder()
        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .unnormalized_coordinates(true);
    let sampler_frame = unsafe { device.create_sampler(&create_info, None)? };

    // Image view for the image for the OpenGL frame.
    let create_info = vk::ImageViewCreateInfo::builder()
        .image(image_frame)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(vk::Format::R8G8B8A8_UNORM)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });
    let image_view_frame = unsafe { device.create_image_view(&create_info, None)? };

    // Image for the sampling buffer.
    let create_info = vk::ImageCreateInfo {
        image_type: vk::ImageType::TYPE_2D,
        format: vk::Format::R16G16B16A16_UNORM,
        extent: vk::Extent3D {
            width,
            height,
            depth: 1,
        },
        mip_levels: 1,
        array_layers: 1,
        samples: vk::SampleCountFlags::TYPE_1,
        tiling: vk::ImageTiling::OPTIMAL,
        usage: vk::ImageUsageFlags::STORAGE // For updating during the sampling stage.
            | vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::TRANSFER_SRC
            | vk::ImageUsageFlags::SAMPLED, // For reading during YUV conversion.
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        ..Default::default()
    };
    let image_sample = unsafe { device.create_image(&create_info, None)? };

    let image_sample_memory_requirements =
        unsafe { device.get_image_memory_requirements(image_sample) };
    let image_sample_memory_type_index = find_memorytype_index(
        &image_sample_memory_requirements,
        &memory_properties,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .ok_or_else(|| eyre!("couldn't find image_sample memory type"))?;
    let create_info = vk::MemoryAllocateInfo::builder()
        .allocation_size(image_sample_memory_requirements.size)
        .memory_type_index(image_sample_memory_type_index);
    let image_sample_memory = unsafe { device.allocate_memory(&create_info, None)? };
    unsafe { device.bind_image_memory(image_sample, image_sample_memory, 0)? };

    // Sampler for the image for the sampling buffer.
    let create_info = vk::SamplerCreateInfo::builder()
        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
        .unnormalized_coordinates(true);
    let sampler_sample = unsafe { device.create_sampler(&create_info, None)? };

    // Image view for the image for the sampling buffer.
    let create_info = vk::ImageViewCreateInfo::builder()
        .image(image_sample)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(vk::Format::R16G16B16A16_UNORM)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });
    let image_view_sample = unsafe { device.create_image_view(&create_info, None)? };

    // Semaphore.
    #[cfg(unix)]
    let mut export_semaphore_create_info = vk::ExportSemaphoreCreateInfo::builder().handle_types(
        vk::ExternalSemaphoreHandleTypeFlags::EXTERNAL_SEMAPHORE_HANDLE_TYPE_OPAQUE_FD,
    );
    #[cfg(windows)]
    let mut export_semaphore_create_info = vk::ExportSemaphoreCreateInfo::builder().handle_types(
        vk::ExternalSemaphoreHandleTypeFlags::EXTERNAL_SEMAPHORE_HANDLE_TYPE_OPAQUE_WIN32,
    );
    let create_info =
        vk::SemaphoreCreateInfo::builder().push_next(&mut export_semaphore_create_info);
    let semaphore = unsafe { device.create_semaphore(&create_info, None)? };

    // Export semaphore.
    #[cfg(unix)]
    let external_semaphore_fd_fn = vk::KhrExternalSemaphoreFdFn::load(|name| unsafe {
        mem::transmute(instance.get_device_proc_addr(device.handle(), name.as_ptr()))
    });
    #[cfg(windows)]
    let external_semaphore_win32_fn = vk::KhrExternalSemaphoreWin32Fn::load(|name| unsafe {
        mem::transmute(instance.get_device_proc_addr(device.handle(), name.as_ptr()))
    });

    // Buffer for color conversion shader output.
    let create_info = vk::BufferCreateInfo::builder()
        .size(width as u64 * height as u64 / 2 * 3) // Full-res Y + quarter-res U, V.
        .usage(vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_SRC)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let buffer_color_conversion_output = unsafe { device.create_buffer(&create_info, None)? };

    let buffer_color_conversion_output_memory_requirements =
        unsafe { device.get_buffer_memory_requirements(buffer_color_conversion_output) };
    let buffer_color_conversion_output_memory_type_index = find_memorytype_index(
        &buffer_color_conversion_output_memory_requirements,
        &memory_properties,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )
    .ok_or_else(|| eyre!("couldn't find buffer_color_conversion_output memory type"))?;
    let create_info = vk::MemoryAllocateInfo::builder()
        .allocation_size(buffer_color_conversion_output_memory_requirements.size)
        .memory_type_index(buffer_color_conversion_output_memory_type_index);
    let buffer_color_conversion_output_memory =
        unsafe { device.allocate_memory(&create_info, None)? };
    unsafe {
        device.bind_buffer_memory(
            buffer_color_conversion_output,
            buffer_color_conversion_output_memory,
            0,
        )?
    };

    // Buffer for reading image pixels.
    let create_info = vk::BufferCreateInfo::builder()
        .size(width as u64 * height as u64 / 2 * 3) // Full-res Y + quarter-res U, V.
        .usage(vk::BufferUsageFlags::TRANSFER_DST)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let buffer = unsafe { device.create_buffer(&create_info, None)? };

    let buffer_memory_requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let buffer_memory_type_index = find_memorytype_index(
        &buffer_memory_requirements,
        &memory_properties,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
    )
    .ok_or_else(|| eyre!("couldn't find buffer memory type"))?;
    let create_info = vk::MemoryAllocateInfo::builder()
        .allocation_size(buffer_memory_requirements.size)
        .memory_type_index(buffer_memory_type_index);
    let buffer_memory = unsafe { device.allocate_memory(&create_info, None)? };
    unsafe { device.bind_buffer_memory(buffer, buffer_memory, 0)? };

    // Descriptor set layout for the color conversion shader.
    let bindings = [
        vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
            .build(),
        vk::DescriptorSetLayoutBinding::builder()
            .binding(1)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
            .build(),
    ];
    let create_info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(&bindings);
    let descriptor_set_layout_color_conversion =
        unsafe { device.create_descriptor_set_layout(&create_info, None)? };

    // Descriptor pool.
    let create_info = vk::DescriptorPoolCreateInfo::builder()
        .max_sets(1)
        .pool_sizes(&[
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 1,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 1,
            },
        ]);
    let descriptor_pool = unsafe { device.create_descriptor_pool(&create_info, None)? };

    // Descriptor set.
    let set_layouts = [descriptor_set_layout_color_conversion];
    let create_info = vk::DescriptorSetAllocateInfo::builder()
        .descriptor_pool(descriptor_pool)
        .set_layouts(&set_layouts);
    let descriptor_set_color_conversion =
        unsafe { device.allocate_descriptor_sets(&create_info)?[0] };

    let image_info = vk::DescriptorImageInfo::builder()
        .sampler(sampler_sample)
        .image_view(image_view_sample)
        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    let image_info = [*image_info];
    let image_descriptor_set = vk::WriteDescriptorSet::builder()
        .dst_set(descriptor_set_color_conversion)
        .dst_binding(0)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .image_info(&image_info);
    let buffer_info = vk::DescriptorBufferInfo::builder()
        .buffer(buffer_color_conversion_output)
        .offset(0)
        .range(vk::WHOLE_SIZE);
    let buffer_info = [*buffer_info];
    let buffer_descriptor_set = vk::WriteDescriptorSet::builder()
        .dst_set(descriptor_set_color_conversion)
        .dst_binding(1)
        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
        .buffer_info(&buffer_info);
    unsafe { device.update_descriptor_sets(&[*image_descriptor_set, *buffer_descriptor_set], &[]) };

    // Shader.
    let shader_code = include_bytes!("color_conversion.spv");
    let shader_code = read_spv(&mut Cursor::new(&shader_code[..]))?;

    let create_info = vk::ShaderModuleCreateInfo::builder().code(&shader_code);
    let shader_module = unsafe { device.create_shader_module(&create_info, None)? };

    // Pipeline.
    let set_layouts = [descriptor_set_layout_color_conversion];
    let create_info = vk::PipelineLayoutCreateInfo::builder().set_layouts(&set_layouts);
    let pipeline_layout = unsafe { device.create_pipeline_layout(&create_info, None)? };

    let name = b"main\0";
    let name = unsafe { CStr::from_ptr(name.as_ptr().cast()) };
    let stage = vk::PipelineShaderStageCreateInfo::builder()
        .stage(vk::ShaderStageFlags::COMPUTE)
        .module(shader_module)
        .name(name);
    let create_info = vk::ComputePipelineCreateInfo::builder()
        .stage(*stage)
        .layout(pipeline_layout);
    let pipeline = unsafe {
        device
            .create_compute_pipelines(vk::PipelineCache::null(), &[*create_info], None)
            .map_err(|(_, err)| err)?[0]
    };

    // Release image for the OpenGL frame and signal semaphore.
    let begin_info =
        vk::CommandBufferBeginInfo::builder().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    unsafe { device.begin_command_buffer(command_buffer_sampling, &begin_info)? };

    let image_frame_memory_barrier = vk::ImageMemoryBarrier::builder()
        .src_access_mask(vk::AccessFlags::empty())
        .dst_access_mask(vk::AccessFlags::empty())
        .old_layout(vk::ImageLayout::UNDEFINED)
        .new_layout(vk::ImageLayout::GENERAL)
        .src_queue_family_index(queue_family_index)
        .dst_queue_family_index(vk::QUEUE_FAMILY_EXTERNAL)
        .image(image_frame)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });

    // Transition the sampling buffer image to the correct layout.
    let image_sample_memory_barrier = vk::ImageMemoryBarrier::builder()
        .src_access_mask(vk::AccessFlags::empty())
        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
        .old_layout(vk::ImageLayout::UNDEFINED)
        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image_sample)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });

    unsafe {
        device.cmd_pipeline_barrier(
            command_buffer_sampling,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[*image_frame_memory_barrier, *image_sample_memory_barrier],
        )
    };

    unsafe { device.end_command_buffer(command_buffer_sampling)? };

    let create_info = vk::FenceCreateInfo::default();
    let fence = unsafe { device.create_fence(&create_info, None)? };

    let command_buffers = [command_buffer_sampling];
    let semaphores = [semaphore];
    let submit_info = vk::SubmitInfo::builder()
        .command_buffers(&command_buffers)
        .signal_semaphores(&semaphores);
    unsafe { device.queue_submit(queue, &[*submit_info], fence)? };

    unsafe { device.wait_for_fences(&[fence], true, u64::max_value())? };
    unsafe { device.destroy_fence(fence, None) };

    Ok(Vulkan {
        width,
        height,
        queue_family_index,
        device,
        command_pool,
        command_buffer_sampling,
        command_buffer_color_conversion,
        queue,
        image_frame,
        image_frame_memory,
        image_frame_memory_size: image_frame_memory_requirements.size,
        #[cfg(unix)]
        external_memory_fd,
        #[cfg(windows)]
        external_memory_win32_fn,
        sampler_frame,
        image_view_frame,
        image_sample,
        image_sample_memory,
        sampler_sample,
        image_view_sample,
        semaphore,
        #[cfg(unix)]
        external_semaphore_fd_fn,
        #[cfg(windows)]
        external_semaphore_win32_fn,
        buffer_color_conversion_output,
        buffer_color_conversion_output_memory,
        buffer,
        buffer_memory,
        descriptor_set_layout_color_conversion,
        descriptor_pool,
        descriptor_set_color_conversion,
        shader_module,
        pipeline_layout,
        pipeline,
    })
}

// https://github.com/MaikKlein/ash/blob/8d7dfee763733a17f4644397458b7391696a990c/examples/src/lib.rs#L239-L272
fn find_memorytype_index(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    // Try to find an exactly matching memory flag
    let best_suitable_index =
        find_memorytype_index_f(memory_req, memory_prop, flags, |property_flags, flags| {
            property_flags == flags
        });
    if best_suitable_index.is_some() {
        return best_suitable_index;
    }
    // Otherwise find a memory flag that works
    find_memorytype_index_f(memory_req, memory_prop, flags, |property_flags, flags| {
        property_flags & flags == flags
    })
}

fn find_memorytype_index_f<F: Fn(vk::MemoryPropertyFlags, vk::MemoryPropertyFlags) -> bool>(
    memory_req: &vk::MemoryRequirements,
    memory_prop: &vk::PhysicalDeviceMemoryProperties,
    flags: vk::MemoryPropertyFlags,
    f: F,
) -> Option<u32> {
    let mut memory_type_bits = memory_req.memory_type_bits;
    for (index, ref memory_type) in memory_prop.memory_types.iter().enumerate() {
        if memory_type_bits & 1 == 1 && f(memory_type.property_flags, flags) {
            return Some(index as u32);
        }
        memory_type_bits >>= 1;
    }
    None
}
