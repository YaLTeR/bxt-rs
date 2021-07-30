use ash::version::{EntryV1_0, InstanceV1_0};
use ash::vk;
use color_eyre::eyre;
use once_cell::sync::OnceCell;

pub static VULKAN: OnceCell<Vulkan> = OnceCell::new();

pub struct Vulkan {
    #[allow(unused)] // Required to keep the library loaded.
    entry: ash::Entry,
    instance: ash::Instance,
    #[cfg(feature = "vulkan-debug")]
    debug_utils: ash::extensions::ext::DebugUtils,
    #[cfg(feature = "vulkan-debug")]
    debug_messenger: vk::DebugUtilsMessengerEXT,
}

impl Drop for Vulkan {
    fn drop(&mut self) {
        unsafe {
            #[cfg(feature = "vulkan-debug")]
            self.debug_utils
                .destroy_debug_utils_messenger(self.debug_messenger, None);
            self.instance.destroy_instance(None);
        }
    }
}

impl Vulkan {
    fn new() -> eyre::Result<Self> {
        // Entry, instance.
        let entry = ash::Entry::new()?;
        let app_info = vk::ApplicationInfo {
            // A few extensions we need for capturing are core in 1.1.
            api_version: vk::make_version(1, 1, 0),
            ..Default::default()
        };
        let extension_names = [
            #[cfg(feature = "vulkan-debug")]
            ash::extensions::ext::DebugUtils::name().as_ptr(),
        ];
        let layer_names = [
            #[cfg(feature = "vulkan-debug")]
            b"VK_LAYER_KHRONOS_validation\0".as_ptr().cast(),
        ];
        let create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extension_names)
            .enabled_layer_names(&layer_names);
        let instance = unsafe { entry.create_instance(&create_info, None)? };

        // Debug utils.
        #[cfg(feature = "vulkan-debug")]
        let debug_utils = ash::extensions::ext::DebugUtils::new(&entry, &instance);
        #[cfg(feature = "vulkan-debug")]
        let create_info = vk::DebugUtilsMessengerCreateInfoEXT {
            message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
            message_type: vk::DebugUtilsMessageTypeFlagsEXT::all(),
            pfn_user_callback: Some(debug_utils_callback),
            ..Default::default()
        };
        #[cfg(feature = "vulkan-debug")]
        let debug_messenger =
            unsafe { debug_utils.create_debug_utils_messenger(&create_info, None)? };

        Ok(Self {
            entry,
            instance,
            #[cfg(feature = "vulkan-debug")]
            debug_utils,
            #[cfg(feature = "vulkan-debug")]
            debug_messenger,
        })
    }

    pub fn instance(&self) -> &ash::Instance {
        &self.instance
    }
}

#[cfg(feature = "vulkan-debug")]
unsafe extern "system" fn debug_utils_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let message = std::ffi::CStr::from_ptr((*p_callback_data).p_message).to_string_lossy();
    info!("{:?} {:?} {}", message_severity, message_types, message);
    vk::FALSE
}

pub fn init() {
    if VULKAN.get().is_some() {
        return;
    }

    let vulkan = match Vulkan::new() {
        Ok(vulkan) => vulkan,
        Err(err) => {
            warn!("error loading Vulkan: {:?}", err);
            return;
        }
    };

    let _ = VULKAN.set(vulkan);
}
