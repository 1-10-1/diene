#![allow(dead_code)]

use std::{ffi::CString, ops::Deref};

use ash::{
    ext::debug_utils,
    khr::swapchain,
    vk::{self, DebugUtilsObjectNameInfoEXT, PhysicalDevice, PhysicalDeviceProperties},
};
use common::logging::macros::*;
use thiserror::Error;

use super::VulkanBackend;
use crate::renderer::backend::{
    instance::{self, VulkanInstance},
    surface::{self, VulkanSurface},
};

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub enum VulkanDeviceError {
    /// Queue-family index did not fit in Vulkan's `u32` index type.
    #[error("queue family index {index} does not fit in u32")]
    QueueFamilyIndexOverflow { index: usize },

    /// Vulkan API call returned an error value.
    #[error("vulkan result has an error value: {0}")]
    UnexpectedResult(ash::vk::Result),
}

pub(super) struct VulkanDevice {
    debug_utils_loader: debug_utils::Device,
    raw: ash::Device,
    physical: PhysicalDevice,
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
            .map_err(VulkanDeviceError::UnexpectedResult)?;

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

                        let Ok(queue_family_index) = u32::try_from(index) else {
                            return Some(Err(VulkanDeviceError::QueueFamilyIndexOverflow {
                                index,
                            }));
                        };

                        // SAFETY: `pdevice` came from `instance`, `surface` was created for the
                        // same instance, and `index` comes from this
                        // physical device's queue-family list.
                        match unsafe {
                            surface.get_loader().get_physical_device_surface_support(
                                *pdevice,
                                queue_family_index,
                                surface.get(),
                            )
                        } {
                            Ok(true) => Some(Ok((*pdevice, queue_family_index))),
                            Ok(false) => None,
                            Err(err) => Some(Err(VulkanDeviceError::UnexpectedResult(err))),
                        }
                    })
            })
            .transpose()?
            .ok_or(VulkanDeviceError::UnexpectedResult(vk::Result::ERROR_INITIALIZATION_FAILED))?;

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

        let mut device = VulkanDevice {
            debug_utils_loader,
            raw,
            physical: pdevice,
            #[cfg(debug_assertions)]
            name: c"Untitled".to_owned(),
        };

        #[cfg(debug_assertions)]
        device.set_name(c"Logical Device".to_owned())?;

        Err(VulkanDeviceError::QueueFamilyIndexOverflow { index: 2 })
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
) -> Result<PhysicalDevice, VulkanDeviceError> {
    // SAFETY: `instance` owns a valid Vulkan instance for the duration of device
    // selection.
    let pdevices = unsafe { inst.enumerate_physical_devices() }
        .map_err(VulkanDeviceError::UnexpectedResult)?;

    for device in pdevices {
        if find_queue_family_indices(inst, device, surf)?.is_some() {
            return Ok(device);
        }
    }

    Err(VulkanDeviceError::UnexpectedResult(vk::Result::ERROR_INITIALIZATION_FAILED))
}

fn find_queue_family_indices(
    inst: &instance::VulkanInstance,
    device: PhysicalDevice,
    surf: &surface::VulkanSurface,
) -> Result<Option<QueueFamilyIndices>, VulkanDeviceError> {
    let main_flags = vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE;
    let queue_index = |index| {
        u32::try_from(index).map_err(|_| VulkanDeviceError::QueueFamilyIndexOverflow { index })
    };

    let mut main: Option<u32> = None;
    let mut present: Option<u32> = None;
    let mut transfer: Option<u32> = None;

    // SAFETY: `device` came from `inst`, so querying its queue families against
    // the same instance is valid.
    let qfs = unsafe { inst.get_physical_device_queue_family_properties(device) };

    for (index, queue_family) in qfs.iter().enumerate() {
        let index = queue_index(index)?;
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
        .map_err(VulkanDeviceError::UnexpectedResult)?
            && (present.is_none() || Some(index) == main)
        {
            present = Some(index);
        }
    }

    let transfer = transfer
        .map(Ok)
        .or_else(|| {
            qfs.iter()
                .enumerate()
                .find(|(_, queue_family)| {
                    queue_family.queue_flags.contains(vk::QueueFlags::TRANSFER)
                })
                .map(|(index, _)| queue_index(index))
        })
        .transpose()?
        .or(main);

    match (main, present, transfer) {
        (Some(main), Some(present), Some(transfer)) if present == main => {
            Ok(Some(QueueFamilyIndices { main, present, transfer }))
        }
        _ => Ok(None),
    }
}
