#![allow(dead_code)]

use std::{
    ffi::{CStr, CString, c_char},
    fmt::Write,
};

use ash::{
    ext::debug_utils,
    vk::{self, DebugUtilsObjectNameInfoEXT, PhysicalDevice, PhysicalDeviceProperties},
};
use common::logging::macros::*;
use error_stack::{Report, ResultExt};
use thiserror::Error;

use super::VulkanBackend;
use crate::renderer::backend::{
    instance::{self, ApiVersion, VulkanInstance},
    surface::{self, VulkanSurface},
};

const REQUIRED_EXTENSIONS: [&CStr; 1] = [
    vk::KHR_SWAPCHAIN_NAME,
    // For profiling:
    // vk::KHR_CALIBRATED_TIMESTAMPS_NAME,
];

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanDeviceError {
    /// Vulkan API call returned an error value.
    #[error("vulkan result has an error value: {0}")]
    UnexpectedResult(ash::vk::Result),

    /// No physical devices were suitable.
    #[error("transparent")]
    NoSuitablePhysicalDevice,
}

pub(super) struct VulkanDevice {
    debug_utils_loader: debug_utils::Device,
    logical: ash::Device,
    physical: PhysicalDevice,
    properties: PhysicalDeviceProperties,
    graphics_queue: vk::Queue,
    compute_queue: vk::Queue,
    transfer_queue: vk::Queue,
    dedicated_compute: bool,
    dedicated_transfer: bool,
    #[cfg(debug_assertions)]
    name: CString,
}

pub(super) struct QueueFamilyIndices {
    graphics: u32,
    present: u32,
    compute: u32,
    transfer: u32,
}

struct DeviceCandidate {
    physical: PhysicalDevice,
    properties: PhysicalDeviceProperties,
    queue_families: QueueFamilyIndices,
    score: u32,
    features_10: vk::PhysicalDeviceFeatures,
    features_11: vk::PhysicalDeviceVulkan11Features<'static>,
    features_12: vk::PhysicalDeviceVulkan12Features<'static>,
    features_13: vk::PhysicalDeviceVulkan13Features<'static>,
}

impl VulkanDevice {
    pub(super) fn get(&self) -> &ash::Device {
        &self.logical
    }

    pub(super) fn get_physical(&self) -> PhysicalDevice {
        self.physical
    }

    #[cfg(debug_assertions)]
    pub(super) fn get_name(&self) -> &CString {
        &self.name
    }

    #[cfg(debug_assertions)]
    pub(super) fn set_name<T: vk::Handle>(
        &self,
        name: &CString,
        handle: T,
    ) -> core::result::Result<(), vk::Result> {
        let name_info =
            DebugUtilsObjectNameInfoEXT::default().object_name(name).object_handle(handle);

        // SAFETY: `self.logical` is a live device, `debug_utils_loader` was created for it, and
        // `name_info` points to `self.name`, which lives throughout the entire
        // lifetime of this struct.
        unsafe { self.debug_utils_loader.set_debug_utils_object_name(&name_info)? };

        Ok(())
    }
}

impl Drop for VulkanDevice {
    fn drop(&mut self) {
        // SAFETY: `self.logical` is a valid logical device created by `create_device`,
        // owned exclusively by this RAII wrapper, and destroyed exactly once here.
        // Future device-owned resources must be destroyed before this wrapper drops.
        unsafe {
            self.logical.destroy_device(None);
        }

        trace!("device destroyed");
    }
}

impl std::fmt::Debug for VulkanDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Use debug names here
        f.debug_struct("<Vulkan Device>").finish()
    }
}

impl VulkanBackend {
    /// Creates the Vulkan logical device.
    pub(super) fn create_device(
        instance: &VulkanInstance,
        surface: &VulkanSurface,
    ) -> error_stack::Result<VulkanDevice, VulkanDeviceError> {
        let DeviceCandidate {
            queue_families,
            physical,
            properties,
            score,
            features_10,
            mut features_11,
            mut features_12,
            mut features_13,
        } = pick_physical(instance, surface)?;

        let priorities = [1.0];

        let mut unique_queue_families = Vec::with_capacity(4);

        for queue_family in [
            queue_families.graphics,
            queue_families.present,
            queue_families.compute,
            queue_families.transfer,
        ] {
            if !unique_queue_families.contains(&queue_family) {
                unique_queue_families.push(queue_family);
            }
        }

        let queue_create_infos: Vec<vk::DeviceQueueCreateInfo<'_>> = unique_queue_families
            .into_iter()
            .map(|queue_family| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(queue_family)
                    .queue_priorities(&priorities)
            })
            .collect();

        let req_exts =
            REQUIRED_EXTENSIONS.iter().map(|ext| ext.as_ptr()).collect::<Vec<*const c_char>>();

        let mut features = vk::PhysicalDeviceFeatures2::default()
            .features(features_10)
            .push_next(&mut features_11)
            .push_next(&mut features_12)
            .push_next(&mut features_13);

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&req_exts)
            .push_next(&mut features);

        // SAFETY: `physical` was selected from `instance`, and `device_create_info` only references
        // local data that lives through this call.
        let logical = unsafe { instance.get().create_device(physical, &device_create_info, None) }
            .map_err(|result| Report::new(VulkanDeviceError::UnexpectedResult(result)))
            .attach_printable("failed to create vulkan logical device")?;

        let debug_utils_loader = debug_utils::Device::new(instance.get(), &logical);

        // SAFETY: Queue family index represents a valid queue family, as `instance.create_device`
        // succeeded with the given queue create infos.
        let graphics_queue = unsafe { logical.get_device_queue(queue_families.graphics, 0) };

        // SAFETY: Queue family index represents a valid queue family, as `instance.create_device`
        // succeeded with the given queue create infos.
        let compute_queue = unsafe { logical.get_device_queue(queue_families.compute, 0) };

        // SAFETY: Queue family index represents a valid queue family, as `instance.create_device`
        // succeeded with the given queue create infos.
        let transfer_queue = unsafe { logical.get_device_queue(queue_families.transfer, 0) };

        let device = VulkanDevice {
            debug_utils_loader,
            logical,
            physical,
            properties,
            graphics_queue,
            compute_queue,
            transfer_queue,
            dedicated_compute: queue_families.compute != queue_families.graphics,
            dedicated_transfer: queue_families.transfer != queue_families.graphics,
            #[cfg(debug_assertions)]
            name: c"Untitled".to_owned(),
        };

        #[cfg(debug_assertions)]
        device
            .set_name(&c"Logical Device".to_owned(), device.logical.handle())
            .map_err(|result| Report::new(VulkanDeviceError::UnexpectedResult(result)))?;

        let queue_sharing_label = |q1: u32, q2: u32| {
            if q1 == q2 {
                "aliased"
            } else {
                "dedicated"
            }
        };

        // SAFETY: Vulkan guarantees that `properties.device_name` is a
        // null-terminated UTF-8 string.
        let physical_name = unsafe { CStr::from_ptr(properties.device_name.as_ptr()) }
            .to_string_lossy()
            .into_owned();

        debug!(
            "Created logical device with physical device {physical_name} (score: \
             {score})\nCompute queue: {}\nTransfer queue: {}\nPresent queue: {}",
            queue_sharing_label(queue_families.graphics, queue_families.compute),
            queue_sharing_label(queue_families.graphics, queue_families.transfer),
            queue_sharing_label(queue_families.graphics, queue_families.present),
        );

        Ok(device)
    }
}

fn pick_physical(
    inst: &instance::VulkanInstance,
    surf: &surface::VulkanSurface,
) -> error_stack::Result<DeviceCandidate, VulkanDeviceError> {
    // SAFETY: `instance` owns a valid Vulkan instance for the duration of device
    // selection.
    let pdevices = unsafe { inst.get().enumerate_physical_devices() }
        .map_err(|result| Report::new(VulkanDeviceError::UnexpectedResult(result)))
        .attach_printable("failed to enumerate physical devices")?;

    let mut best_candidate: Option<DeviceCandidate> = None;

    for physical in pdevices {
        let mut props2 = vk::PhysicalDeviceProperties2::default();

        // SAFETY: `device` descends from `inst`, so this is valid.
        unsafe { inst.get().get_physical_device_properties2(physical, &mut props2) };

        let properties = props2.properties;

        // SAFETY: Vulkan guarantees that `props.device_name` is a
        // null-terminated UTF-8 string.
        let name = unsafe { CStr::from_ptr(properties.device_name.as_ptr()) }
            .to_string_lossy()
            .into_owned();

        let mut failure_log_buf = String::new();

        let _ = writeln!(
            failure_log_buf,
            "Graphics card {name} was rejected due to the following failed conditions:"
        );

        let api_version: ApiVersion = properties.api_version.into();

        if api_version < instance::MIN_API_VERSION {
            let _ = writeln!(
                failure_log_buf,
                "\t- Minimum API version not supported (Required {}, found {api_version})",
                instance::MIN_API_VERSION
            );

            trace!("{failure_log_buf}");

            continue;
        }

        let mut queried_features_10 = vk::PhysicalDeviceFeatures::default();
        let mut queried_features_11 = vk::PhysicalDeviceVulkan11Features::default();
        let mut queried_features_12 = vk::PhysicalDeviceVulkan12Features::default();
        let mut queried_features_13 = vk::PhysicalDeviceVulkan13Features::default();

        {
            let mut features = vk::PhysicalDeviceFeatures2::default()
                .features(queried_features_10)
                .push_next(&mut queried_features_11)
                .push_next(&mut queried_features_12)
                .push_next(&mut queried_features_13);

            // SAFETY: `device` descends from `inst`, so this is valid.
            unsafe { inst.get().get_physical_device_features2(physical, &mut features) };

            queried_features_10 = features.features;

            queried_features_11.p_next = core::ptr::null_mut();
            queried_features_12.p_next = core::ptr::null_mut();
            queried_features_13.p_next = core::ptr::null_mut();
        }

        let mut conds_met = true;

        let Some(queue_families) = find_queue_family_indices(inst, physical, surf)
            .attach_printable("failed to query queue family indices")?
        else {
            let _ = writeln!(failure_log_buf, "\t- Necessary queues absent");

            trace!("{failure_log_buf}");

            continue;
        };

        let features_info = get_features_info(
            &queried_features_10,
            &queried_features_11,
            &queried_features_12,
            &queried_features_13,
        );

        for (cond, met) in features_info.availability.into_iter().chain(vec![(
            "Necessary extensions supported",
            check_device_extension_support(inst, physical)?,
        )]) {
            if !met {
                conds_met = false;

                let _ = writeln!(failure_log_buf, "\t- {cond}");
            }
        }

        if !conds_met {
            trace!("{failure_log_buf}");
            continue;
        }

        let mut score = 0;

        if properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
            score += 1000;
        }

        score += properties.limits.max_image_dimension2_d;

        let candidate = DeviceCandidate {
            physical,
            properties,
            queue_families,
            score,
            features_10: features_info.enabled_f10,
            features_11: features_info.enabled_f11,
            features_12: features_info.enabled_f12,
            features_13: features_info.enabled_f13,
        };

        if let Some(cand) = &best_candidate
            && score < cand.score
        {
            continue;
        }

        best_candidate = Some(candidate);
    }

    best_candidate.ok_or_else(|| {
        Report::new(VulkanDeviceError::NoSuitablePhysicalDevice)
            .attach_printable("no suitable physical device was found")
    })
}

fn check_device_extension_support(
    inst: &instance::VulkanInstance,
    device: vk::PhysicalDevice,
) -> error_stack::Result<bool, VulkanDeviceError> {
    // SAFETY: `device` descends from `inst`, so this is valid.
    let available_exts = unsafe {
        inst.get()
            .enumerate_device_extension_properties(device)
            .map_err(VulkanDeviceError::UnexpectedResult)?
    };

    Ok(REQUIRED_EXTENSIONS.iter().all(|required| {
        available_exts.iter().any(|ext| {
            // SAFETY: Vulkan guarantees that `ext.extension_name` is a
            // valid null-terminated string.
            let available = unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) };

            available == *required
        })
    }))
}

fn find_queue_family_indices(
    inst: &instance::VulkanInstance,
    device: PhysicalDevice,
    surf: &surface::VulkanSurface,
) -> error_stack::Result<Option<QueueFamilyIndices>, VulkanDeviceError> {
    let mut graphics: Option<u32> = None;
    let mut graphics_present: Option<u32> = None;
    let mut present: Option<u32> = None;
    let mut compute: Option<u32> = None;
    let mut compute_is_dedicated = false;
    let mut transfer_dedicated: Option<u32> = None;
    let mut transfer_non_graphics: Option<u32> = None;
    let mut transfer_any: Option<u32> = None;

    // SAFETY: `device` came from `inst`, so querying its queue families against
    // the same instance is valid.
    let qfs = unsafe { inst.get().get_physical_device_queue_family_properties(device) };

    for (index, queue_family) in qfs.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
        let index = index as u32;

        let flags = queue_family.queue_flags;
        let has_graphics = flags.contains(vk::QueueFlags::GRAPHICS);
        let has_compute = flags.contains(vk::QueueFlags::COMPUTE);
        let has_transfer = flags.contains(vk::QueueFlags::TRANSFER);

        if has_graphics {
            graphics.get_or_insert(index);
        }

        if has_compute {
            let dedicated = !has_graphics;

            if compute.is_none() || (dedicated && !compute_is_dedicated) {
                compute = Some(index);
                compute_is_dedicated = dedicated;
            }
        }

        if has_transfer {
            transfer_any.get_or_insert(index);

            if !has_graphics {
                transfer_non_graphics.get_or_insert(index);
            }

            if !has_graphics && !has_compute {
                transfer_dedicated.get_or_insert(index);
            }
        }

        // SAFETY: `device` came from `inst`, `surf` was created for the same
        // instance, and `index` comes from this physical device's queue families.
        let supports_present = unsafe {
            surf.get_loader().get_physical_device_surface_support(device, index, surf.get())
        }
        .map_err(|result| Report::new(VulkanDeviceError::UnexpectedResult(result)))
        .attach_printable("failed to query physical device surface support")?;

        if supports_present {
            present.get_or_insert(index);

            if has_graphics {
                graphics_present.get_or_insert(index);
            }
        }
    }

    let graphics = graphics_present.or(graphics);
    let present = graphics_present.or(present);

    let transfer =
        transfer_dedicated.or(transfer_non_graphics).or(transfer_any).or(compute).or(graphics);

    // NOTE: We're currently forcing graphics == present here.
    Ok(match (graphics, present, compute, transfer) {
        (Some(graphics), Some(present), Some(compute), Some(transfer)) if graphics == present => {
            Some(QueueFamilyIndices { graphics, present, compute, transfer })
        }
        _ => None,
    })
}

#[derive(Default)]
struct FeaturesInfo {
    availability: Vec<(&'static str, bool)>,
    enabled_f10: vk::PhysicalDeviceFeatures,
    enabled_f11: vk::PhysicalDeviceVulkan11Features<'static>,
    enabled_f12: vk::PhysicalDeviceVulkan12Features<'static>,
    enabled_f13: vk::PhysicalDeviceVulkan13Features<'static>,
}

macro_rules! require_features {
    (
        $availability:ident;
        $(
            $supported:ident => $enabled:ident {
                $(
                    $field:ident => $label:literal
                ),* $(,)?
            }
        )*
    ) => {
        $(
            $(
                let available = $supported.$field != vk::FALSE;
                $enabled.$field = if available { vk::TRUE } else { vk::FALSE };
                $availability.push(($label, available));
            )*
        )*
    };
}

fn get_features_info(
    supported_10: &vk::PhysicalDeviceFeatures,
    supported_11: &vk::PhysicalDeviceVulkan11Features<'_>,
    supported_12: &vk::PhysicalDeviceVulkan12Features<'_>,
    supported_13: &vk::PhysicalDeviceVulkan13Features<'_>,
) -> FeaturesInfo {
    let mut availability = Vec::new();

    let mut enabled_10 = vk::PhysicalDeviceFeatures::default();
    let mut enabled_11 = vk::PhysicalDeviceVulkan11Features::default();
    let mut enabled_12 = vk::PhysicalDeviceVulkan12Features::default();
    let mut enabled_13 = vk::PhysicalDeviceVulkan13Features::default();

    require_features!(availability;
        supported_10 => enabled_10 {
            geometry_shader => "Geometry shader availability",
            sampler_anisotropy => "Anisotropy availability",
            sample_rate_shading => "Sample rate shading availability",
            multi_draw_indirect => "Multi-draw indirect availability",
            fill_mode_non_solid => "Non-solid fill mode availability",
            vertex_pipeline_stores_and_atomics => "Vertex pipeline stores and atomics availability",
            fragment_stores_and_atomics => "Fragment stores and atomics availability",
            shader_storage_image_multisample => "Storage image multisample availability",
            shader_int64 => "64-bit integer shader support availability",
        }

        supported_11 => enabled_11 {
            shader_draw_parameters => "Shader draw parameters availability",
        }

        supported_12 => enabled_12 {
            storage_buffer8_bit_access => "8-bit storage buffer access availability",
            descriptor_indexing => "Descriptor indexing availability",
            shader_sampled_image_array_non_uniform_indexing => "Non-uniform sampled image array indexing availability",
            descriptor_binding_partially_bound => "Partially bound descriptor binding availability",
            runtime_descriptor_array => "Runtime descriptor array availability",
            timeline_semaphore => "Timeline semaphore availability",
            buffer_device_address => "Buffer device address availability",
            vulkan_memory_model => "Vulkan memory model availability",
            vulkan_memory_model_device_scope => "Vulkan memory model device scope availability",
        }

        supported_13 => enabled_13 {
            synchronization2 => "Synchronization2 availability",
            dynamic_rendering => "Dynamic rendering availability",
        }
    );

    FeaturesInfo {
        availability,
        enabled_f10: enabled_10,
        enabled_f11: enabled_11,
        enabled_f12: enabled_12,
        enabled_f13: enabled_13,
    }
}
