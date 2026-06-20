use std::{
    borrow::Cow,
    ffi::{CStr, c_char},
    ops::Deref,
};

use ash::{
    ext::debug_utils,
    vk::{self, DebugUtilsMessageSeverityFlagsEXT},
};
use common::logging::macros::*;
use raw_window_handle::RawDisplayHandle;
use thiserror::Error;

use super::VulkanBackend;

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub enum VulkanInstanceError {
    /// Vulkan API call returned an error value.
    #[error("vulkan result has an error value: [{0:?}] {0}")]
    UnexpectedResult(ash::vk::Result),
}

pub(super) struct VulkanInstance {
    debug_callback: vk::DebugUtilsMessengerEXT,
    debug_utils_loader: debug_utils::Instance,
    raw: ash::Instance,
}

impl VulkanInstance {
    pub(super) fn get(&self) -> &ash::Instance {
        &self.raw
    }
}

impl Drop for VulkanInstance {
    fn drop(&mut self) {
        unsafe {
            self.debug_utils_loader.destroy_debug_utils_messenger(self.debug_callback, None);
        }

        trace!("debug messenger destroyed");

        unsafe {
            self.raw.destroy_instance(None);
        }

        trace!("instance destroyed");
    }
}

impl Deref for VulkanInstance {
    type Target = ash::Instance;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl std::fmt::Debug for VulkanInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Use debug names here
        f.debug_struct("<Vulkan Instance>").finish()
    }
}

impl VulkanBackend {
    /// Creates the Vulkan instance.
    pub(super) fn create_instance(entry: &ash::Entry, display_handle: RawDisplayHandle) -> Result<VulkanInstance, VulkanInstanceError> {
        let layer_names = [c"VK_LAYER_KHRONOS_validation"];
        let layers_names_raw: Vec<*const c_char> = layer_names.iter().map(|raw_name| raw_name.as_ptr()).collect();

        let mut extension_names = ash_window::enumerate_required_extensions(display_handle)
            .map_err(VulkanInstanceError::UnexpectedResult)?
            .to_vec();

        extension_names.push(debug_utils::NAME.as_ptr());

        let appinfo = vk::ApplicationInfo::default()
            .application_name(c"Diene Vulkan Backend")
            .application_version(0)
            .engine_name(c"Diene")
            .engine_version(0)
            .api_version(vk::make_api_version(0, 1, 4, 0));

        let mut debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING)
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .pfn_user_callback(Some(vulkan_debug_callback));

        let raw = {
            let create_info = vk::InstanceCreateInfo::default()
                .application_info(&appinfo)
                .enabled_layer_names(&layers_names_raw)
                .enabled_extension_names(&extension_names)
                .flags(vk::InstanceCreateFlags::default())
                .push_next(&mut debug_info);

            unsafe { entry.create_instance(&create_info, None).map_err(VulkanInstanceError::UnexpectedResult)? }
        };

        trace!("instance initialized");

        let debug_utils_loader = debug_utils::Instance::new(entry, &raw);
        let debug_callback = unsafe {
            debug_utils_loader
                .create_debug_utils_messenger(&debug_info, None)
                .map_err(VulkanInstanceError::UnexpectedResult)?
        };

        trace!("debug messenger initialized");

        Ok(VulkanInstance { debug_callback, debug_utils_loader, raw })
    }
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = unsafe { *p_callback_data };
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        unsafe { CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy() }
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        unsafe { CStr::from_ptr(callback_data.p_message).to_string_lossy() }
    };

    let msg = format!("[{message_type:?}] {message_id_name} ({message_id_number}):\n{message}");

    match message_severity {
        DebugUtilsMessageSeverityFlagsEXT::INFO => info!("{}", msg),
        DebugUtilsMessageSeverityFlagsEXT::WARNING => warn!("{}", msg),
        DebugUtilsMessageSeverityFlagsEXT::VERBOSE => trace!("{}", msg),
        DebugUtilsMessageSeverityFlagsEXT::ERROR => error!("{}", msg),
        _ => (),
    }

    vk::FALSE
}
