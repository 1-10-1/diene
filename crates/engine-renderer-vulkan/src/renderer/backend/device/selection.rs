use std::{ffi::CStr, fmt::Write};

use ash::vk::{self, PhysicalDevice, PhysicalDeviceProperties};
use common::logging::macros::*;

use super::{
    REQUIRED_EXTENSIONS, VulkanDeviceError,
    features::{FeaturesInfo, get_features_info},
    queues::{QueueFamilyIndices, find_queue_family_indices},
};
use crate::renderer::backend::{
    instance::{self, ApiVersion, VulkanInstance},
    surface::VulkanSurface,
};

pub(super) struct DeviceCandidate {
    pub(super) physical: PhysicalDevice,
    pub(super) properties: PhysicalDeviceProperties,
    pub(super) queue_families: QueueFamilyIndices,
    pub(super) score: u32,
    pub(super) features_10: vk::PhysicalDeviceFeatures,
    pub(super) features_11: vk::PhysicalDeviceVulkan11Features<'static>,
    pub(super) features_12: vk::PhysicalDeviceVulkan12Features<'static>,
    pub(super) features_13: vk::PhysicalDeviceVulkan13Features<'static>,
}

pub(super) fn pick_physical(
    inst: &VulkanInstance,
    surf: &VulkanSurface,
) -> core::result::Result<DeviceCandidate, VulkanDeviceError> {
    // SAFETY: `inst` owns a valid Vulkan instance for the duration of device
    // selection.
    let physical_devices =
        vk_try!("enumerate physical devices", unsafe { inst.get().enumerate_physical_devices() });

    let mut best_candidate: Option<DeviceCandidate> = None;

    for physical in physical_devices {
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

        let Some(queue_families) = find_queue_family_indices(inst, physical, surf)? else {
            let _ = writeln!(failure_log_buf, "\t- Necessary queues absent");

            trace!("{failure_log_buf}");

            continue;
        };

        let FeaturesInfo { mut availability, enabled_f10, enabled_f11, enabled_f12, enabled_f13 } =
            get_features_info(
                &queried_features_10,
                &queried_features_11,
                &queried_features_12,
                &queried_features_13,
            );

        let extensions_supported = check_device_extension_support(inst, physical)?;
        availability.push(("Necessary extensions supported", extensions_supported));

        if extensions_supported {
            availability.push((
                "Swapchain surface support adequate",
                check_swapchain_adequacy(surf, physical)?,
            ));
        }

        let mut conditions_met = true;

        for (condition, met) in availability {
            if !met {
                conditions_met = false;

                let _ = writeln!(failure_log_buf, "\t- {condition}");
            }
        }

        if !conditions_met {
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
            features_10: enabled_f10,
            features_11: enabled_f11,
            features_12: enabled_f12,
            features_13: enabled_f13,
        };

        if let Some(current) = &best_candidate
            && score < current.score
        {
            continue;
        }

        best_candidate = Some(candidate);
    }

    best_candidate.ok_or(VulkanDeviceError::NoSuitablePhysicalDevice)
}

fn check_device_extension_support(
    inst: &VulkanInstance,
    device: vk::PhysicalDevice,
) -> core::result::Result<bool, VulkanDeviceError> {
    // SAFETY: `device` descends from `inst`, so this is valid.
    let available_exts = vk_try!("enumerate device extension properties", unsafe {
        inst.get().enumerate_device_extension_properties(device)
    });

    Ok(REQUIRED_EXTENSIONS.iter().all(|required| {
        available_exts.iter().any(|ext| {
            // SAFETY: Vulkan guarantees that `ext.extension_name` is a
            // valid null-terminated string.
            let available = unsafe { CStr::from_ptr(ext.extension_name.as_ptr()) };

            available == *required
        })
    }))
}

fn check_swapchain_adequacy(
    surf: &VulkanSurface,
    device: vk::PhysicalDevice,
) -> core::result::Result<bool, VulkanDeviceError> {
    // SAFETY: `surf` is a live surface created from the same instance as the physical device.
    let capabilities = vk_try!("query surface capabilities for device selection", unsafe {
        surf.get_loader().get_physical_device_surface_capabilities(device, surf.get())
    });

    // SAFETY: `surf` is a live surface created from the same instance as the physical device.
    let formats = vk_try!("query surface formats for device selection", unsafe {
        surf.get_loader().get_physical_device_surface_formats(device, surf.get())
    });

    // SAFETY: `surf` is a live surface created from the same instance as the physical device.
    let present_modes = vk_try!("query present modes for device selection", unsafe {
        surf.get_loader().get_physical_device_surface_present_modes(device, surf.get())
    });

    let supports_known_composite_alpha = [
        vk::CompositeAlphaFlagsKHR::OPAQUE,
        vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
        vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
        vk::CompositeAlphaFlagsKHR::INHERIT,
    ]
    .into_iter()
    .any(|mode| capabilities.supported_composite_alpha.contains(mode));

    Ok(!formats.is_empty()
        && !present_modes.is_empty()
        && capabilities.supported_usage_flags.contains(vk::ImageUsageFlags::COLOR_ATTACHMENT)
        && supports_known_composite_alpha)
}
