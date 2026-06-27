#![allow(dead_code)]

use std::{
    collections::HashSet,
    ffi::{CStr, CString, c_char},
    fmt::Write,
    ops::Deref,
};

use ash::{
    ext::debug_utils,
    vk::{self, DebugUtilsObjectNameInfoEXT, PhysicalDevice, PhysicalDeviceProperties},
};
use common::logging::macros::*;
use error_stack::{Report, Result, ResultExt};
use thiserror::Error;

use super::VulkanBackend;
use crate::renderer::backend::{
    instance::{self, VulkanInstance},
    surface::{self, VulkanSurface},
};

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
    raw: ash::Device,
    physical: PhysicalDevice,
    properties: PhysicalDeviceProperties,
    main_queue: vk::Queue,
    present_queue: vk::Queue,
    transfer_queue: vk::Queue,
    #[cfg(debug_assertions)]
    name: CString,
}

impl VulkanDevice {
    pub(super) fn get(&self) -> &ash::Device {
        &self.raw
    }

    pub(super) fn get_physical(&self) -> PhysicalDevice {
        self.physical
    }

    #[cfg(debug_assertions)]
    pub(super) fn get_name(&self) -> &CString {
        &self.name
    }

    #[cfg(debug_assertions)]
    pub(super) fn set_name(&mut self, name: CString) -> Result<(), VulkanDeviceError> {
        self.name = name;

        let name_info = DebugUtilsObjectNameInfoEXT::default()
            .object_name(&self.name[..])
            .object_handle(self.raw.handle());

        // SAFETY: `raw` is a live device, `debug_utils_loader` was created for it, and
        // `name_info` points to `self.name`, which lives throughout the entire
        // lifetime of this struct.
        unsafe { self.debug_utils_loader.set_debug_utils_object_name(&name_info) }
            .map_err(|result| Report::new(VulkanDeviceError::UnexpectedResult(result)))
            .attach_printable("failed to set logical device debug name")?;

        Ok(())
    }
}

impl Drop for VulkanDevice {
    fn drop(&mut self) {
        // SAFETY: `self.raw` is a valid logical device created by `create_device`,
        // owned exclusively by this RAII wrapper, and destroyed exactly once here.
        // No custom allocator was used at creation, so `None` is passed again.
        // Future device-owned resources must be destroyed before this wrapper drops.
        unsafe {
            self.raw.destroy_device(None);
        }

        trace!("device destroyed");
    }
}

impl Deref for VulkanDevice {
    type Target = ash::Device;

    fn deref(&self) -> &Self::Target {
        &self.raw
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
    ) -> Result<VulkanDevice, VulkanDeviceError> {
        let DeviceCandidate {
            indices: queue_families,
            device: physical,
            props: properties,
            score: _,
        } = pick_physical(instance, surface)?;

        let features = vk::PhysicalDeviceFeatures { shader_clip_distance: 1, ..Default::default() };

        let priorities = [1.0];

        let queue_create_infos: Vec<vk::DeviceQueueCreateInfo<'_>> =
            [queue_families.main, queue_families.present, queue_families.transfer]
                .into_iter()
                .map(|i| {
                    vk::DeviceQueueCreateInfo::default()
                        .queue_family_index(i)
                        .queue_priorities(&priorities)
                })
                .collect();

        println!("{queue_create_infos:#?}");

        let req_exts =
            REQUIRED_EXTENSIONS.iter().map(|ext| ext.as_ptr()).collect::<Vec<*const c_char>>();

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&req_exts)
            .enabled_features(&features);

        // SAFETY: `pdevice` and `queue_family_index` were selected from `instance`,
        // and `device_create_info` only references local data that lives through this call.
        let raw = unsafe { instance.create_device(physical, &device_create_info, None) }
            .map_err(|result| Report::new(VulkanDeviceError::UnexpectedResult(result)))
            .attach_printable("failed to create vulkan logical device")?;

        let debug_utils_loader = debug_utils::Device::new(instance, &raw);

        // SAFETY: Queue family index represents a valid queue family, as `instance.create_device`
        // succeeded with the given queue create infos.
        let main_queue = unsafe { raw.get_device_queue(queue_families.main, 0) };

        // SAFETY: Queue family index represents a valid queue family, as `instance.create_device`
        // succeeded with the given queue create infos.
        let transfer_queue = unsafe { raw.get_device_queue(queue_families.transfer, 0) };

        // SAFETY: Queue family index represents a valid queue family, as `instance.create_device`
        // succeeded with the given queue create infos.
        let present_queue = unsafe { raw.get_device_queue(queue_families.present, 0) };

        let mut device = VulkanDevice {
            debug_utils_loader,
            raw,
            physical,
            properties,
            main_queue,
            transfer_queue,
            present_queue,
            #[cfg(debug_assertions)]
            name: c"Untitled".to_owned(),
        };

        #[cfg(debug_assertions)]
        device.set_name(c"Logical Device".to_owned())?;

        Ok(device)
    }
}

pub(super) struct QueueFamilyIndices {
    main: u32,
    present: u32,
    transfer: u32,
}

struct DeviceCandidate {
    device: PhysicalDevice,
    props: PhysicalDeviceProperties,
    indices: QueueFamilyIndices,
    score: u32,
}

fn pick_physical(
    inst: &instance::VulkanInstance,
    surf: &surface::VulkanSurface,
) -> Result<DeviceCandidate, VulkanDeviceError> {
    // SAFETY: `instance` owns a valid Vulkan instance for the duration of device
    // selection.
    let pdevices = unsafe { inst.enumerate_physical_devices() }
        .map_err(|result| Report::new(VulkanDeviceError::UnexpectedResult(result)))
        .attach_printable("failed to enumerate physical devices")?;

    let mut best_candidate: Option<DeviceCandidate> = None;

    for device in pdevices {
        let mut props2 = vk::PhysicalDeviceProperties2::default();

        // SAFETY: `device` descends from `inst`, so this is valid.
        unsafe { inst.get_physical_device_properties2(device, &mut props2) };

        let props = props2.properties;

        let mut vk11_features = vk::PhysicalDeviceVulkan11Features::default();
        let mut vk12_features = vk::PhysicalDeviceVulkan12Features::default();

        let mut vk13_features = vk::PhysicalDeviceVulkan13Features::default();

        let mut extended_dynamic_state_features =
            vk::PhysicalDeviceExtendedDynamicStateFeaturesEXT::default();

        let mut shader_draw_parameters_features =
            vk::PhysicalDeviceShaderDrawParametersFeatures::default();

        let mut features = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut vk11_features)
            .push_next(&mut vk12_features)
            .push_next(&mut vk13_features)
            .push_next(&mut extended_dynamic_state_features)
            .push_next(&mut shader_draw_parameters_features);

        // SAFETY: `device` descends from `inst`, so this is valid.
        unsafe { inst.get_physical_device_features2(device, &mut features) };

        {
            let mut conds_met = true;

            // SAFETY: Vulkan guarantees that `props.device_name` is a
            // null-terminated UTF-8 string.
            let name = unsafe { CStr::from_ptr(props.device_name.as_ptr()) }
                .to_string_lossy()
                .into_owned();

            let mut failure_log_buf = String::new();

            #[allow(clippy::unwrap_used)]
            writeln!(
                failure_log_buf,
                "Graphics card {name} was rejected due to the following failed conditions:"
            )
            .unwrap();

            let Some(queue_family_indices) = find_queue_family_indices(inst, device, surf)
                .attach_printable("failed to query queue family indices")?
            else {
                #[allow(clippy::unwrap_used)]
                writeln!(failure_log_buf, "\tNecessary queues present").unwrap();

                continue;
            };

            for (cond, met) in get_feature_requirements(
                &features.features,
                &vk11_features,
                &vk12_features,
                &vk13_features,
                &extended_dynamic_state_features,
                &shader_draw_parameters_features,
            )
            .into_iter()
            .chain(vec![(
                "Necessary extensions supported",
                check_device_extension_support(inst, device)?,
            )]) {
                if !met {
                    conds_met = false;

                    #[allow(clippy::unwrap_used)]
                    writeln!(failure_log_buf, "\t- {cond}").unwrap();
                }
            }

            if !conds_met {
                trace!("{failure_log_buf}");
                continue;
            }

            let mut score = 0;

            if props.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
                score += 1000;
            }

            score += props.limits.max_image_dimension2_d;

            let candidate = DeviceCandidate { score, device, props, indices: queue_family_indices };

            if let Some(cand) = &best_candidate {
                if score < cand.score {
                    continue;
                }
            }

            best_candidate = Some(candidate);
        }
    }

    best_candidate.ok_or_else(|| {
        Report::new(VulkanDeviceError::UnexpectedResult(vk::Result::ERROR_INITIALIZATION_FAILED))
            .attach_printable("no suitable physical device was found")
    })
}

const REQUIRED_EXTENSIONS: [&CStr; 1] = [
    vk::KHR_SWAPCHAIN_NAME,
    // For profiling:
    // vk::KHR_CALIBRATED_TIMESTAMPS_NAME
];

fn check_device_extension_support(
    inst: &instance::VulkanInstance,
    device: vk::PhysicalDevice,
) -> Result<bool, VulkanDeviceError> {
    // SAFETY: `device` descends from `inst`, so this is valid.
    let available_exts: HashSet<&CStr> = unsafe {
        inst.enumerate_device_extension_properties(device)
            .map_err(VulkanDeviceError::UnexpectedResult)?
    }
    .iter()
    .map(|ext| unsafe {
        // SAFETY: Vulkan guarantees `extension_name` is a null-terminated C string.
        CStr::from_ptr(ext.extension_name.as_ptr())
    })
    .collect();

    Ok(REQUIRED_EXTENSIONS.iter().all(|required| available_exts.contains(*required)))
}

fn find_queue_family_indices(
    inst: &instance::VulkanInstance,
    device: PhysicalDevice,
    surf: &surface::VulkanSurface,
) -> Result<Option<QueueFamilyIndices>, VulkanDeviceError> {
    let main_flags = vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE;

    let mut main: Option<u32> = None;
    let mut present: Option<u32> = None;
    let mut transfer: Option<u32> = None;

    // SAFETY: `device` came from `inst`, so querying its queue families against
    // the same instance is valid.
    let qfs = unsafe { inst.get_physical_device_queue_family_properties(device) };

    for (index, queue_family) in qfs.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
        let index = index as u32;

        let flags = queue_family.queue_flags;

        if flags.contains(main_flags) {
            main.get_or_insert(index);
        }

        if flags.contains(vk::QueueFlags::TRANSFER) && !flags.intersects(main_flags) {
            transfer.get_or_insert(index);
        }

        // SAFETY: `device` came from `inst`, `surf` was created for the same
        // instance, and `index` comes from this physical device's queue families.
        if unsafe {
            surf.get_loader().get_physical_device_surface_support(device, index, surf.get())
        }
        .map_err(|result| Report::new(VulkanDeviceError::UnexpectedResult(result)))
        .attach_printable("failed to query physical device surface support")?
            && (present.is_none() || Some(index) == main)
        {
            present = Some(index);
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::as_conversions)]
    let transfer = transfer
        .or_else(|| {
            qfs.iter()
                .position(|queue_family| {
                    queue_family.queue_flags.contains(vk::QueueFlags::TRANSFER)
                })
                .map(|index| index as u32)
        })
        .or(main);

    match (main, present, transfer) {
        (Some(main), Some(present), Some(transfer)) if present == main => {
            Ok(Some(QueueFamilyIndices { main, present, transfer }))
        }
        _ => Ok(None),
    }
}

fn get_feature_requirements(
    f10: &vk::PhysicalDeviceFeatures,
    _f11: &vk::PhysicalDeviceVulkan11Features<'_>,
    f12: &vk::PhysicalDeviceVulkan12Features<'_>,
    f13: &vk::PhysicalDeviceVulkan13Features<'_>,
    ds: &vk::PhysicalDeviceExtendedDynamicStateFeaturesEXT<'_>,
    sdp: &vk::PhysicalDeviceShaderDrawParametersFeatures<'_>,
) -> Vec<(&'static str, bool)> {
    vec![
        ("Geometry shader availability", f10.geometry_shader),
        ("Anisotropy availability", f10.sampler_anisotropy),
        ("Sample rate shading availability", f10.sample_rate_shading),
        ("Multi-draw indirect availability", f10.multi_draw_indirect),
        ("Non-solid fill mode availability", f10.fill_mode_non_solid),
        ("Vertex pipeline stores and atomics availability", f10.vertex_pipeline_stores_and_atomics),
        ("Fragment stores and atomics availability", f10.fragment_stores_and_atomics),
        ("Storage image multisample availability", f10.shader_storage_image_multisample),
        ("64-bit integer shader support availability", f10.shader_int64),
        ("8-bit storage buffer access availability", f12.storage_buffer8_bit_access),
        ("Descriptor indexing availability", f12.descriptor_indexing),
        (
            "Non-uniform sampled image array indexing availability",
            f12.shader_sampled_image_array_non_uniform_indexing,
        ),
        ("Partially bound descriptor binding availability", f12.descriptor_binding_partially_bound),
        ("Runtime descriptor array availability", f12.runtime_descriptor_array),
        ("Timeline semaphore availability", f12.timeline_semaphore),
        ("Buffer device address availability", f12.buffer_device_address),
        ("Vulkan memory model availability", f12.vulkan_memory_model),
        ("Vulkan memory model device scope availability", f12.vulkan_memory_model_device_scope),
        ("Synchronization2 availability", f13.synchronization2),
        ("Dynamic rendering availability", f13.dynamic_rendering),
        ("Extended dynamic state availability", ds.extended_dynamic_state),
        ("Shader draw parameters availability", sdp.shader_draw_parameters),
    ]
    .into_iter()
    .map(|(s, b)| (s, b != 0))
    .collect()
}
