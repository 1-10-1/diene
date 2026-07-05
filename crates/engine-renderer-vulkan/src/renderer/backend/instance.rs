#[cfg(debug_assertions)]
use std::borrow::Cow;
use std::ffi::{CStr, c_char};

#[cfg(debug_assertions)]
use ash::ext::debug_utils;
use ash::vk;
use common::logging::macros::*;
use raw_window_handle::RawDisplayHandle;
use thiserror::Error;

use crate::renderer::backend::call_error::VulkanCallError;

pub(super) const MIN_API_VERSION: ApiVersion = ApiVersion::new(1, 3, 0);

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct ApiVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

impl ApiVersion {
    pub(super) const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
}

impl From<u32> for ApiVersion {
    fn from(version: u32) -> Self {
        Self {
            major: vk::api_version_major(version),
            minor: vk::api_version_minor(version),
            patch: vk::api_version_patch(version),
        }
    }
}

impl From<ApiVersion> for u32 {
    fn from(val: ApiVersion) -> Self {
        vk::make_api_version(0, val.major, val.minor, val.patch)
    }
}

impl std::fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "V{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanInstanceError {
    /// Vulkan API call returned an error value.
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),

    #[error("insufficient vulkan API version: got {got}, wanted {expected}")]
    InsufficientVersion { expected: ApiVersion, got: ApiVersion },
}

pub(super) struct VulkanInstance {
    #[cfg(debug_assertions)]
    debug_callback: Option<vk::DebugUtilsMessengerEXT>,

    #[cfg(debug_assertions)]
    debug_utils_loader: Option<debug_utils::Instance>,

    handle: ash::Instance,
}

impl VulkanInstance {
    pub(super) fn get(&self) -> &ash::Instance {
        &self.handle
    }
}

impl Drop for VulkanInstance {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        {
            if let (Some(callback), Some(debug_utils_loader)) =
                (self.debug_callback.take(), self.debug_utils_loader.take())
            {
                // SAFETY: `callback` was created from `debug_utils_loader` for this
                // instance and is destroyed before the instance itself.
                unsafe {
                    debug_utils_loader.destroy_debug_utils_messenger(callback, None);
                }

                trace!("debug messenger destroyed");
            }
        }

        // SAFETY: `self.handle` is a valid instance owned by this wrapper, all
        // instance children held by this wrapper have been destroyed, and no
        // custom allocator was used.
        unsafe {
            self.handle.destroy_instance(None);
        }

        trace!("instance destroyed");
    }
}

impl std::fmt::Debug for VulkanInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("<Vulkan Instance>").finish()
    }
}

impl VulkanInstance {
    /// Creates the Vulkan instance.
    pub(super) fn new(
        entry: &ash::Entry,
        display_handle: RawDisplayHandle,
    ) -> core::result::Result<Self, VulkanInstanceError> {
        // SAFETY: `entry` was loaded successfully and this call only queries loader-supported
        // instance API version information.
        let supported_version = vk_try!("enumerate Vulkan instance version", unsafe {
            entry.try_enumerate_instance_version()
        });

        if let Some(version) = supported_version.map(ApiVersion::from) {
            if version < MIN_API_VERSION {
                return Err(VulkanInstanceError::InsufficientVersion {
                    expected: MIN_API_VERSION,
                    got: version,
                });
            }
        } else {
            return Err(VulkanInstanceError::InsufficientVersion {
                expected: MIN_API_VERSION,
                got: ApiVersion::new(1, 0, 0),
            });
        }

        #[cfg(debug_assertions)]
        let layer_names: [&'static CStr; 1] = [c"VK_LAYER_KHRONOS_validation"];

        #[cfg(not(debug_assertions))]
        let layer_names: [&'static CStr; 0] = [];

        let layers_names_raw: Vec<*const c_char> =
            layer_names.iter().map(|raw_name| raw_name.as_ptr()).collect();

        let extension_names = {
            let extension_names = vk_try!(
                "enumerate required window-system Vulkan extensions",
                ash_window::enumerate_required_extensions(display_handle),
            )
            .to_vec();

            #[cfg(debug_assertions)]
            {
                let mut extension_names = extension_names;
                extension_names.push(debug_utils::NAME.as_ptr());
                extension_names
            }

            #[cfg(not(debug_assertions))]
            {
                extension_names
            }
        };

        let appinfo = vk::ApplicationInfo::default()
            .application_name(c"Diene Vulkan Backend")
            .application_version(0)
            .engine_name(c"Diene")
            .engine_version(0)
            .api_version(MIN_API_VERSION.into());

        #[cfg(debug_assertions)]
        let mut validation_features = vk::ValidationFeaturesEXT::default()
            .enabled_validation_features(&[
                vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
                vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
            ]);

        #[cfg(debug_assertions)]
        let mut debug_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .pfn_user_callback(Some(vulkan_debug_callback));

        let raw = {
            let create_info = vk::InstanceCreateInfo::default()
                .application_info(&appinfo)
                .enabled_layer_names(&layers_names_raw)
                .enabled_extension_names(&extension_names);

            #[cfg(debug_assertions)]
            let create_info = {
                let mut create_info = create_info;
                create_info =
                    create_info.push_next(&mut debug_info).push_next(&mut validation_features);
                create_info
            };

            // SAFETY: `create_info` points to local data that lives through the
            // call, and no custom allocator is used.
            vk_try!("create Vulkan instance", unsafe { entry.create_instance(&create_info, None) })
        };

        #[cfg(debug_assertions)]
        let mut inst = Self { debug_callback: None, debug_utils_loader: None, handle: raw };

        #[cfg(not(debug_assertions))]
        let inst = Self { handle: raw };

        trace!("instance initialized");

        #[cfg(debug_assertions)]
        {
            let loader = debug_utils::Instance::new(entry, &inst.handle);

            // SAFETY: `debug_info` contains a valid static callback function and
            // lives for the duration of the Vulkan call.
            inst.debug_callback = Some(vk_try!("create Vulkan debug messenger", unsafe {
                loader.create_debug_utils_messenger(&debug_info, None)
            }));

            inst.debug_utils_loader = Some(loader);

            trace!("debug messenger initialized");
        }

        Ok(inst)
    }
}

#[cfg(debug_assertions)]
unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    // SAFETY: Vulkan calls this callback with a valid callback data pointer for
    // the duration of the call.
    let callback_data = unsafe { *p_callback_data };
    let message_id_number = callback_data.message_id_number;

    let message_id_name = if callback_data.p_message_id_name.is_null() {
        Cow::from("")
    } else {
        // SAFETY: Vulkan provides a null-terminated string pointer when this
        // field is non-null, valid for the duration of the callback.
        unsafe { CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy() }
    };

    let message = if callback_data.p_message.is_null() {
        Cow::from("")
    } else {
        // SAFETY: Vulkan provides a null-terminated message pointer when this
        // field is non-null, valid for the duration of the callback.
        unsafe { CStr::from_ptr(callback_data.p_message).to_string_lossy() }
    };

    let msg = format!("[{message_type:?}] {message_id_name} ({message_id_number}):\n{message}");

    match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => info!("{}", msg),
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => warn!("{}", msg),
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => trace!("{}", msg),
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => error!("{}", msg),
        _ => (),
    }

    vk::FALSE
}
