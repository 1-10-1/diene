use std::ffi::{CStr, c_char};
#[cfg(debug_assertions)]
use std::{borrow::Cow, fmt::Write as _};

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
    pub(super) fn handle(&self) -> &ash::Instance {
        &self.handle
    }

    #[cfg(debug_assertions)]
    pub(super) fn _debug_utils_loader(&self) -> Option<&debug_utils::Instance> {
        self.debug_utils_loader.as_ref()
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
    if p_callback_data.is_null() {
        error!("Vulkan debug callback was invoked without callback data");

        return vk::FALSE;
    }

    // SAFETY: Vulkan calls this callback with a valid callback data pointer for
    // the duration of the call.
    let callback_data = unsafe { &*p_callback_data };
    let message_id_number = callback_data.message_id_number;

    let message_id_name = cstr_lossy(callback_data.p_message_id_name, "<unnamed>");
    let message = cstr_lossy(callback_data.p_message, "");

    // SAFETY: These debug-utils arrays are valid for the duration of this callback.
    let queue_labels =
        unsafe { callback_slice(callback_data.p_queue_labels, callback_data.queue_label_count) };

    // SAFETY: These debug-utils arrays are valid for the duration of this callback.
    let command_buffer_labels = unsafe {
        callback_slice(callback_data.p_cmd_buf_labels, callback_data.cmd_buf_label_count)
    };

    // SAFETY: These debug-utils arrays are valid for the duration of this callback.
    let objects = unsafe { callback_slice(callback_data.p_objects, callback_data.object_count) };

    let mut msg = String::new();
    let _ = writeln!(msg, "[{message_type:?}]");
    let _ = writeln!(msg, "  id: {message_id_name} ({message_id_number})");

    if !callback_data.flags.is_empty() {
        let flags = callback_data.flags;
        let _ = writeln!(msg, "  flags: {flags:?}");
    }

    if !message.is_empty() {
        let _ = writeln!(msg, "\n  message:");
        write_indented_lines(&mut msg, &message, 4);
        let _ = writeln!(msg);
    }

    write_labels(&mut msg, "queue labels", queue_labels);
    write_labels(&mut msg, "command buffer labels", command_buffer_labels);
    write_objects(&mut msg, objects);

    match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => info!("{}", msg),
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => warn!("{}", msg),
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => trace!("{}", msg),
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => error!("{}", msg),
        _ => (),
    }

    vk::FALSE
}

#[cfg(debug_assertions)]
fn cstr_lossy<'a>(ptr: *const c_char, fallback: &'static str) -> Cow<'a, str> {
    if ptr.is_null() {
        Cow::Borrowed(fallback)
    } else {
        // SAFETY: Vulkan debug-utils strings are null-terminated and valid for the callback's
        // duration when their pointers are non-null.
        unsafe { CStr::from_ptr(ptr).to_string_lossy() }
    }
}

#[cfg(debug_assertions)]
unsafe fn callback_slice<'a, T>(ptr: *const T, count: u32) -> &'a [T] {
    if ptr.is_null() || count == 0 {
        return &[];
    }

    match usize::try_from(count) {
        Ok(count) => {
            // SAFETY: The validation layer provides `count` elements at `ptr` for the duration of
            // the callback when the pointer is non-null and count is non-zero.
            unsafe { std::slice::from_raw_parts(ptr, count) }
        }
        Err(_) => &[],
    }
}

#[cfg(debug_assertions)]
fn write_indented_lines(buffer: &mut String, message: &str, spaces: usize) {
    for line in message.lines() {
        let _ = writeln!(buffer, "{:spaces$}{line}", "");
    }
}

#[cfg(debug_assertions)]
fn write_labels(buffer: &mut String, title: &str, labels: &[vk::DebugUtilsLabelEXT<'_>]) {
    if labels.is_empty() {
        return;
    }

    let _ = writeln!(buffer, "  {title}:");

    for (index, label) in labels.iter().enumerate() {
        let name = cstr_lossy(label.p_label_name, "<unnamed>");
        let [red, green, blue, alpha] = label.color;

        let _ = writeln!(
            buffer,
            "    {index}: {name} color=rgba({red:.3}, {green:.3}, {blue:.3}, {alpha:.3})",
        );
    }
}

#[cfg(debug_assertions)]
fn write_objects(buffer: &mut String, objects: &[vk::DebugUtilsObjectNameInfoEXT<'_>]) {
    if objects.is_empty() {
        return;
    }

    let _ = writeln!(buffer, "  objects:");

    for (index, object) in objects.iter().enumerate() {
        let object_type = object.object_type;
        let handle = object.object_handle;
        let name = cstr_lossy(object.p_object_name, "<unnamed>");

        let _ = writeln!(buffer, "    {index}: {object_type:?} handle=0x{handle:016x} name={name}");
    }
}
