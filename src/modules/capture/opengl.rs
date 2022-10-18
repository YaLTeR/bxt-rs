use std::ptr::null;

use color_eyre::eyre::{self, eyre, WrapErr};

use super::ExternalObject;
use crate::gl;
use crate::utils::MainThreadMarker;

pub struct OpenGl {
    marker: MainThreadMarker,
    width: i32,
    height: i32,
    memory_object: u32,
    texture: u32,
    semaphore: u32,
    framebuffer: u32,
}

impl Drop for OpenGl {
    fn drop(&mut self) {
        let gl = gl::GL.borrow(self.marker);
        let gl = gl.as_ref().unwrap();

        unsafe {
            gl.DeleteFramebuffers(1, &self.framebuffer);
            gl.DeleteSemaphoresEXT(1, &self.semaphore);
            gl.DeleteTextures(1, &self.texture);
            gl.DeleteMemoryObjectsEXT(1, &self.memory_object);
        }
    }
}

unsafe fn check(gl: &gl::Gl) -> eyre::Result<()> {
    match gl.GetError() {
        gl::NO_ERROR => Ok(()),
        error => Err(eyre!(
            "OpenGL error: {} - {} (0x{:x})",
            match error {
                gl::INVALID_ENUM => "GL_INVALID_ENUM",
                gl::INVALID_VALUE => "GL_INVALID_VALUE",
                gl::INVALID_OPERATION => "GL_INVALID_OPERATION",
                gl::INVALID_FRAMEBUFFER_OPERATION => "GL_INVALID_FRAMEBUFFER_OPERATION",
                gl::STACK_OVERFLOW => "GL_STACK_OVERFLOW",
                gl::STACK_UNDERFLOW => "GL_STACK_UNDERFLOW",
                gl::OUT_OF_MEMORY => "GL_OUT_OF_MEMORY",
                _ => "unknown",
            },
            error,
            error,
        )),
    }
}

macro_rules! check {
    ($gl:expr, $s:stmt) => {{
        $s

        check($gl).wrap_err(stringify!($s))
    }};
}

unsafe fn reset_gl_error(gl: &gl::Gl) {
    while gl.GetError() != gl::NO_ERROR {}
}

impl OpenGl {
    #[instrument(name = "OpenGl::capture", skip_all)]
    pub unsafe fn capture(&self) -> eyre::Result<()> {
        let gl = gl::GL.borrow(self.marker);
        let gl = gl.as_ref().unwrap();

        // HL leaves some GL errors behind.
        reset_gl_error(gl);

        // Acquire ownership from Vulkan.
        check!(
            gl,
            gl.WaitSemaphoreEXT(
                self.semaphore,
                0,
                null(),
                1,
                [self.texture].as_ptr(),
                [gl::LAYOUT_GENERAL_EXT].as_ptr(),
            )
        )?;

        // Save previous bound framebuffer.
        let mut previous_framebuffer = 0;
        check!(
            gl,
            gl.GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut previous_framebuffer)
        )?;

        // Capture.
        check!(
            gl,
            gl.BindFramebuffer(gl::DRAW_FRAMEBUFFER, self.framebuffer)
        )?;
        check!(
            gl,
            gl.FramebufferTexture2D(
                gl::DRAW_FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                self.texture,
                0,
            )
        )?;
        check!(
            gl,
            gl.BlitFramebuffer(
                0,
                0,
                self.width,
                self.height,
                0,
                0,
                self.width,
                self.height,
                gl::COLOR_BUFFER_BIT,
                gl::NEAREST,
            )
        )?;

        // Restore bound framebuffer.
        check!(
            gl,
            gl.BindFramebuffer(gl::DRAW_FRAMEBUFFER, previous_framebuffer as u32)
        )?;

        // Transfer ownership to Vulkan.
        check!(
            gl,
            gl.SignalSemaphoreEXT(
                self.semaphore,
                0,
                null(),
                1,
                [self.texture].as_ptr(),
                [gl::LAYOUT_GENERAL_EXT].as_ptr(),
            )
        )?;

        // This is required, otherwise the semaphore isn't "initialized" until much later.
        check!(gl, gl.Flush())?;

        Ok(())
    }
}

#[instrument(name = "OpenGl::init", skip(marker))]
pub unsafe fn init(
    marker: MainThreadMarker,
    width: i32,
    height: i32,
    size: u64,
    external_image_frame_memory: ExternalObject,
    external_semaphore: ExternalObject,
) -> eyre::Result<OpenGl> {
    let gl = gl::GL.borrow(marker);
    let gl = gl.as_ref().unwrap();

    // HL leaves some GL errors behind.
    reset_gl_error(gl);

    let mut memory_object = 0;
    check!(gl, gl.CreateMemoryObjectsEXT(1, &mut memory_object))?;
    check!(
        gl,
        gl.MemoryObjectParameterivEXT(memory_object, gl::DEDICATED_MEMORY_OBJECT_EXT, &1)
    )?;

    #[cfg(unix)]
    check!(
        gl,
        gl.ImportMemoryFdEXT(
            memory_object,
            size,
            gl::HANDLE_TYPE_OPAQUE_FD_EXT,
            external_image_frame_memory,
        )
    )?;
    #[cfg(windows)]
    check!(
        gl,
        gl.ImportMemoryWin32HandleEXT(
            memory_object,
            size,
            gl::HANDLE_TYPE_OPAQUE_WIN32_EXT,
            external_image_frame_memory,
        )
    )?;

    let mut texture = 0;
    check!(gl, gl.GenTextures(1, &mut texture))?;

    // Save previous bound texture.
    let mut previous_texture = 0;
    check!(
        gl,
        gl.GetIntegerv(gl::TEXTURE_BINDING_2D, &mut previous_texture)
    )?;

    check!(gl, gl.BindTexture(gl::TEXTURE_2D, texture))?;
    check!(
        gl,
        gl.TexStorageMem2DEXT(
            gl::TEXTURE_2D,
            1,
            gl::RGBA8,
            width,
            height,
            memory_object,
            0,
        )
    )?;

    // Restore bound texture.
    check!(gl, gl.BindTexture(gl::TEXTURE_2D, previous_texture as u32))?;

    let mut semaphore = 0;
    check!(gl, gl.GenSemaphoresEXT(1, &mut semaphore))?;

    #[cfg(unix)]
    check!(
        gl,
        gl.ImportSemaphoreFdEXT(semaphore, gl::HANDLE_TYPE_OPAQUE_FD_EXT, external_semaphore)
    )?;
    #[cfg(windows)]
    check!(
        gl,
        gl.ImportSemaphoreWin32HandleEXT(
            semaphore,
            gl::HANDLE_TYPE_OPAQUE_WIN32_EXT,
            external_semaphore,
        )
    )?;

    let mut framebuffer = 0;
    check!(gl, gl.GenFramebuffers(1, &mut framebuffer))?;

    Ok(OpenGl {
        marker,
        width,
        height,
        memory_object,
        texture,
        semaphore,
        framebuffer,
    })
}

#[instrument(name = "OpenGl::capture_with_read_pixels", skip_all)]
pub unsafe fn capture_with_read_pixels(
    marker: MainThreadMarker,
    width: i32,
    height: i32,
    buf: &mut [u8],
) -> eyre::Result<()> {
    assert_eq!(buf.len(), width as usize * height as usize * 3);

    let gl = gl::GL.borrow(marker);
    let gl = gl.as_ref().unwrap();

    // HL leaves some GL errors behind.
    reset_gl_error(gl);

    check!(
        gl,
        gl.ReadPixels(
            0,
            0,
            width,
            height,
            gl::BGR,
            gl::UNSIGNED_BYTE,
            buf.as_mut_ptr().cast()
        )
    )?;

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uuids {
    pub device_uuids: Vec<[u8; 16]>,
    pub driver_uuid: [u8; 16],
}

pub unsafe fn get_uuids(marker: MainThreadMarker) -> eyre::Result<Uuids> {
    let gl = gl::GL.borrow(marker);
    let gl = gl.as_ref().unwrap();

    // HL leaves some GL errors behind.
    reset_gl_error(gl);

    let mut num_uuids = 0;
    check!(gl, gl.GetIntegerv(gl::NUM_DEVICE_UUIDS_EXT, &mut num_uuids))?;

    let mut device_uuids = vec![[0; 16]; num_uuids as _];

    for i in 0..num_uuids {
        check!(
            gl,
            gl.GetUnsignedBytei_vEXT(
                gl::DEVICE_UUID_EXT,
                i as _,
                device_uuids[i as usize].as_mut_ptr()
            )
        )?;
    }

    let mut driver_uuid = [0; 16];
    check!(
        gl,
        gl.GetUnsignedBytevEXT(gl::DRIVER_UUID_EXT, driver_uuid.as_mut_ptr())
    )?;

    let uuids = Uuids {
        device_uuids,
        driver_uuid,
    };
    debug!("OpenGL UUIDs: {:?}", uuids);

    Ok(uuids)
}
