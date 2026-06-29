use ash::vk;
use error_stack::Report;
use thiserror::Error;

use crate::renderer::backend::device;

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanCommandError {
    /// Vulkan API call returned an error value.
    #[error("vulkan result has an error value: {0}")]
    UnexpectedResult(ash::vk::Result),
}

pub(super) struct VulkanCommand {
    graphics_pool: ash::vk::CommandPool,
    transfer_pool: ash::vk::CommandPool,
    compute_pool: ash::vk::CommandPool,
    graphics_command_buffers: Vec<ash::vk::CommandBuffer>,
}

impl VulkanCommand {
    pub(super) fn new(
        device: &device::VulkanDevice,
    ) -> error_stack::Result<Self, VulkanCommandError> {
        // SAFETY: `device` is alive.
        let graphics_pool = unsafe {
            device.get().create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(device.get_queue_families().graphics),
                None,
            )
        }
        .map_err(|result| Report::new(VulkanCommandError::UnexpectedResult(result)))?;

        // SAFETY: `device` is alive.
        let transfer_pool = unsafe {
            device.get().create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(device.get_queue_families().transfer),
                None,
            )
        }
        .map_err(|result| Report::new(VulkanCommandError::UnexpectedResult(result)))?;

        // SAFETY: `device` is alive.
        let compute_pool = unsafe {
            device.get().create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(device.get_queue_families().compute),
                None,
            )
        }
        .map_err(|result| Report::new(VulkanCommandError::UnexpectedResult(result)))?;

        // SAFETY: `device` is alive.
        let graphics_command_buffers = unsafe {
            device.get().allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(graphics_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }
        .map_err(|result| Report::new(VulkanCommandError::UnexpectedResult(result)))?;

        Ok(Self { graphics_pool, transfer_pool, compute_pool, graphics_command_buffers })
    }
}
