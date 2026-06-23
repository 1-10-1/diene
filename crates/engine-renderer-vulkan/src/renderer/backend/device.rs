#![allow(dead_code)]

use std::ffi::CString;

use ash::{
    ext::debug_utils,
    khr::swapchain,
    vk::{self, DebugUtilsObjectNameInfoEXT, PhysicalDevice},
};
use common::logging::macros::*;
use thiserror::Error;

use super::VulkanBackend;
use crate::renderer::backend::{instance::VulkanInstance, surface::VulkanSurface};

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub enum VulkanDeviceError {
    #[error("unexpected error")]
    Unexpected,

    /// Vulkan API call returned an error value.
    #[error("vulkan result has an error value: {0}")]
    UnexpectedResult(ash::vk::Result),
}

pub(super) struct VulkanDevice {
    debug_utils_loader: debug_utils::Device,
    raw: ash::Device,
    physical: PhysicalDevice,
    #[cfg(debug_assertions)]
    name: String,
}

impl VulkanDevice {
    pub(super) fn get(&self) -> &ash::Device {
        &self.raw
    }

    pub(super) fn get_physical(&self) -> PhysicalDevice {
        self.physical
    }

    #[cfg(debug_assertions)]
    pub(super) fn get_name(&self) -> &String {
        &self.name
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

impl std::fmt::Debug for VulkanDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Use debug names here
        f.debug_struct("<Vulkan Device>").finish()
    }
}

pub(super) struct QueueFamilyIndices {
    main: u32,
    present: u32,
    transfer: u32,
}

const INVALID_QUEUE_FAMILY_INDEX: u32 = u32::MAX;

impl VulkanBackend {
    /// Creates the Vulkan logical device.
    pub(super) fn create_device(
        instance: &VulkanInstance,
        surface: &VulkanSurface,
    ) -> Result<VulkanDevice, VulkanDeviceError> {
        trace!("device initialized");
        // SAFETY: `instance` owns a valid Vulkan instance for the duration of device
        // selection.
        let pdevices = unsafe { instance.enumerate_physical_devices() }
            .map_err(VulkanDeviceError::UnexpectedResult)?;

        let (pdevice, queue_family_index) = pdevices
            .iter()
            .find_map(|pdevice| {
                // SAFETY: `pdevice` came from `instance`, so querying its queue families
                // against the same instance is valid.
                unsafe { instance.get_physical_device_queue_family_properties(*pdevice) }
                    .iter()
                    .enumerate()
                    .find_map(|(index, info)| {
                        if !info.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                            return None;
                        }

                        // SAFETY: `pdevice` came from `instance`, `surface` was created for the
                        // same instance, and `index` comes from this
                        // physical device's queue-family list.
                        match unsafe {
                            surface.get_loader().get_physical_device_surface_support(
                                *pdevice,
                                #[allow(clippy::expect_used)]
                                index.try_into().expect("unexpected index overflow on cast to u32"),
                                surface.get(),
                            )
                        } {
                            Ok(true) => Some(Ok((*pdevice, index))),
                            Ok(false) => None,
                            Err(err) => Some(Err(VulkanDeviceError::UnexpectedResult(err))),
                        }
                    })
            })
            .transpose()?
            .ok_or(VulkanDeviceError::UnexpectedResult(vk::Result::ERROR_INITIALIZATION_FAILED))?;

        let queue_family_index =
            queue_family_index.try_into().map_err(|_| VulkanDeviceError::Unexpected)?;

        let device_extension_names_raw = [swapchain::NAME.as_ptr()];
        let features = vk::PhysicalDeviceFeatures { shader_clip_distance: 1, ..Default::default() };
        let priorities = [1.0];

        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&priorities);

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_info))
            .enabled_extension_names(&device_extension_names_raw)
            .enabled_features(&features);

        // SAFETY: `pdevice` and `queue_family_index` were selected from `instance`,
        // and `device_create_info` only references local data that lives through this call.
        let raw = unsafe { instance.create_device(pdevice, &device_create_info, None) }
            .map_err(VulkanDeviceError::UnexpectedResult)?;

        let debug_utils_loader = debug_utils::Device::new(instance, &raw);

        #[allow(clippy::unwrap_used)]
        let name = CString::new("Logical Device").unwrap();

        let device = VulkanDevice {
            debug_utils_loader,
            raw,
            physical: pdevice,
            name: name.to_string_lossy().into_owned(),
        };

        let name_info = DebugUtilsObjectNameInfoEXT::default()
            .object_name(name.as_c_str())
            .object_handle(device.raw.handle());

        // SAFETY: `raw` is a live device, `debug_utils_loader` was created for it, and
        // `name_info` points to `name`, which lives through this call.
        unsafe { device.debug_utils_loader.set_debug_utils_object_name(&name_info) }
            .map_err(VulkanDeviceError::UnexpectedResult)?;

        Ok(device)
    }
}
