use thiserror::Error;
use vk_mem::{AllocatorCreateFlags, AllocatorCreateInfo};

use crate::renderer::backend::{device, instance};

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanAllocatorError {
    /// Vulkan API call returned an error value.
    #[error("vulkan result has an error value: {0}")]
    UnexpectedResult(ash::vk::Result),
}

pub(super) struct VulkanAllocator {
    handle: vk_mem::Allocator,
}

impl VulkanAllocator {
    pub(super) fn new(
        instance: &instance::VulkanInstance,
        device: &device::VulkanDevice,
    ) -> error_stack::Result<Self, VulkanAllocatorError> {
        let mut create_info =
            AllocatorCreateInfo::new(instance.get(), device.get(), device.get_physical());

        create_info.flags = AllocatorCreateFlags::BUFFER_DEVICE_ADDRESS;

        // SAFETY: All three arguments will outlive this allocator.
        let handle = unsafe { vk_mem::Allocator::new(create_info) }
            .map_err(VulkanAllocatorError::UnexpectedResult)?;

        Ok(Self { handle })
    }
}
