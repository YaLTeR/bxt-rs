use std::env;
use std::os::raw::c_char;

use ash::vk;
use color_eyre::eyre;
use once_cell::sync::Lazy;

pub static VULKAN: Lazy<Option<Vulkan>> = Lazy::new(|| match Vulkan::new() {
    Ok(vulkan) => Some(vulkan),
    Err(err) => {
        warn!("error loading Vulkan: {:?}", err);
        None
    }
});

pub struct Vulkan {
    #[allow(unused)] // Required to keep the library loaded.
    entry: ash::Entry,
    instance: ash::Instance,
    debug: Option<(ash::extensions::ext::DebugUtils, vk::DebugUtilsMessengerEXT)>,
}

impl Drop for Vulkan {
    fn drop(&mut self) {
        unsafe {
            if let Some((utils, messenger)) = self.debug.take() {
                utils.destroy_debug_utils_messenger(messenger, None);
            }
            self.instance.destroy_instance(None);
        }
    }
}

impl Vulkan {
    #[instrument("Vulkan::new")]
    fn new() -> eyre::Result<Self> {
        debug!("initializing Vulkan");

        // Entry, instance.
        let entry = unsafe { ash::Entry::load()? };
        let app_info = vk::ApplicationInfo {
            // A few extensions we need for capturing are core in 1.1.
            api_version: vk::make_api_version(0, 1, 1, 0),
            ..Default::default()
        };

        let mut create_info = vk::InstanceCreateInfo::builder().application_info(&app_info);
        if env::var_os("BXT_RS_VULKAN_DEBUG").is_some() {
            const EXTENSION_NAMES: [*const c_char; 1] =
                [ash::extensions::ext::DebugUtils::name().as_ptr()];
            const LAYER_NAMES: [*const c_char; 1] =
                [b"VK_LAYER_KHRONOS_validation\0".as_ptr().cast()];

            create_info = create_info
                .enabled_extension_names(&EXTENSION_NAMES)
                .enabled_layer_names(&LAYER_NAMES);
        }

        let instance = unsafe { entry.create_instance(&create_info, None)? };

        let debug = if env::var_os("BXT_RS_VULKAN_DEBUG").is_some() {
            // Debug utils.
            let debug_utils = ash::extensions::ext::DebugUtils::new(&entry, &instance);
            let create_info = vk::DebugUtilsMessengerCreateInfoEXT {
                message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
                message_type: vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                pfn_user_callback: Some(debug_utils_callback),
                ..Default::default()
            };
            let debug_messenger =
                unsafe { debug_utils.create_debug_utils_messenger(&create_info, None)? };

            Some((debug_utils, debug_messenger))
        } else {
            None
        };

        Ok(Self {
            entry,
            instance,
            debug,
        })
    }

    pub fn instance(&self) -> &ash::Instance {
        &self.instance
    }
}

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
