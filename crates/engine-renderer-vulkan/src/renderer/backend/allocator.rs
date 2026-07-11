use thiserror::Error;
use vk_mem::{AllocatorCreateFlags, AllocatorCreateInfo};

use crate::renderer::backend::{call_error::VulkanCallError, device, instance};

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanAllocatorError {
    /// Vulkan API call returned an error value.
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),
}

#[allow(dead_code)]
pub(super) struct VulkanAllocator {
    handle: vk_mem::Allocator,
}

impl VulkanAllocator {
    pub(super) fn new(
        instance: &instance::VulkanInstance,
        device: &device::VulkanLogicalDevice,
        physical_device: ash::vk::PhysicalDevice,
    ) -> core::result::Result<Self, VulkanAllocatorError> {
        let mut create_info =
            AllocatorCreateInfo::new(instance.handle(), device.handle(), physical_device);

        create_info.vulkan_api_version = instance::MIN_API_VERSION.into();

        create_info.flags = AllocatorCreateFlags::BUFFER_DEVICE_ADDRESS;

        // SAFETY: All three arguments will outlive this allocator.
        let handle = vk_try!("create Vulkan memory allocator", unsafe {
            vk_mem::Allocator::new(create_info)
        });

        Ok(Self { handle })
    }
}
