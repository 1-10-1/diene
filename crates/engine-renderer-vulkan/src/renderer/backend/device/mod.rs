mod features;
mod logical;
mod queues;
mod selection;

use std::{
    ffi::{CStr, c_char},
    sync::Arc,
};

use ash::vk::{self, PhysicalDevice, PhysicalDeviceProperties};
use common::logging::macros::*;
use thiserror::Error;

use self::selection::{DeviceCandidate, pick_physical};
pub(super) use self::{logical::VulkanLogicalDevice, queues::QueueFamilyIndices};
use crate::renderer::backend::{
    call_error::VulkanCallError, instance::VulkanInstance, surface::VulkanSurface,
};

const REQUIRED_EXTENSIONS: [&CStr; 1] = [
    vk::KHR_SWAPCHAIN_NAME,
    // For profiling:
    // vk::KHR_CALIBRATED_TIMESTAMPS_NAME,
];

/// Errors returned by Vulkan device operations.
#[derive(Debug, Error)]
pub(super) enum VulkanDeviceError {
    /// Vulkan API call returned an error value.
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),

    /// No physical devices were suitable.
    #[error("no physical devices were found suitable for this renderer.")]
    NoSuitablePhysicalDevice,
}

#[allow(dead_code)]
pub(super) struct VulkanDevice {
    logical: Arc<VulkanLogicalDevice>,
    physical: PhysicalDevice,
    properties: PhysicalDeviceProperties,
    graphics_queue: vk::Queue,
    compute_queue: vk::Queue,
    transfer_queue: vk::Queue,
    compute_separate_from_graphics: bool,
    transfer_separate_from_graphics: bool,
    queue_families: QueueFamilyIndices,
}

impl VulkanDevice {
    /// Creates the physical/logical device pair used by the renderer.
    pub(super) fn new(
        instance: &VulkanInstance,
        surface: &VulkanSurface,
    ) -> core::result::Result<Self, VulkanDeviceError> {
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
        let logical_handle = vk_try!("create logical device", unsafe {
            instance.get().create_device(physical, &device_create_info, None)
        });

        let logical = Arc::new(VulkanLogicalDevice::new(instance.get(), logical_handle));

        // SAFETY: Queue family index represents a valid queue family, as `instance.create_device`
        // succeeded with the given queue create infos.
        let graphics_queue =
            unsafe { logical.get_handle().get_device_queue(queue_families.graphics, 0) };

        // SAFETY: Queue family index represents a valid queue family, as `instance.create_device`
        // succeeded with the given queue create infos.
        let compute_queue =
            unsafe { logical.get_handle().get_device_queue(queue_families.compute, 0) };

        // SAFETY: Queue family index represents a valid queue family, as `instance.create_device`
        // succeeded with the given queue create infos.
        let transfer_queue =
            unsafe { logical.get_handle().get_device_queue(queue_families.transfer, 0) };

        let device = Self {
            logical,
            physical,
            properties,
            graphics_queue,
            compute_queue,
            transfer_queue,
            compute_separate_from_graphics: queue_families.compute != queue_families.graphics,
            transfer_separate_from_graphics: queue_families.transfer != queue_families.graphics,
            queue_families: queue_families.clone(),
        };

        #[cfg(debug_assertions)]
        vk_try!(
            "name logical device",
            device.logical.set_name(c"Logical Device", device.logical.get_handle().handle()),
        );

        let queue_sharing_label = |q1: u32, q2: u32| {
            if q1 == q2 {
                "shared with graphics"
            } else {
                "separate from graphics"
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

    pub(super) fn logical(&self) -> &Arc<VulkanLogicalDevice> {
        &self.logical
    }

    pub(super) fn physical(&self) -> PhysicalDevice {
        self.physical
    }

    pub(super) fn queue_families(&self) -> &QueueFamilyIndices {
        &self.queue_families
    }
}
