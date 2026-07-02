use std::sync::Arc;

use ash::vk::{self, CommandPool, Handle};
use error_stack::Report;
use thiserror::Error;

use crate::renderer::backend::device::{self, QueueFamilyIndices};

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanCommandError {
    /// Vulkan API call returned an error value.
    #[error("vulkan result has an error value: {0}")]
    UnexpectedResult(ash::vk::Result),
}

#[allow(dead_code)]
pub(super) struct VulkanCommand {
    graphics_pool: ash::vk::CommandPool,
    transfer_pool: ash::vk::CommandPool,
    compute_pool: ash::vk::CommandPool,
    graphics_command_buffers: Vec<ash::vk::CommandBuffer>,
    device: Arc<device::VulkanLogicalDevice>,
}

impl Drop for VulkanCommand {
    fn drop(&mut self) {
        // SAFETY: `self.device` is alive.
        unsafe {
            if !self.graphics_pool.is_null() {
                self.device.get_handle().destroy_command_pool(self.graphics_pool, None);
                self.graphics_pool = CommandPool::null();
            }

            if !self.transfer_pool.is_null() {
                self.device.get_handle().destroy_command_pool(self.transfer_pool, None);
                self.transfer_pool = CommandPool::null();
            }

            if !self.compute_pool.is_null() {
                self.device.get_handle().destroy_command_pool(self.compute_pool, None);
                self.compute_pool = CommandPool::null();
            }
        }
    }
}

impl VulkanCommand {
    pub(super) fn new(
        device: Arc<device::VulkanLogicalDevice>,
        queue_families: &QueueFamilyIndices,
    ) -> error_stack::Result<Self, VulkanCommandError> {
        let mut command = Self {
            graphics_pool: CommandPool::default(),
            transfer_pool: CommandPool::default(),
            compute_pool: CommandPool::default(),
            graphics_command_buffers: Vec::default(),
            device,
        };

        // SAFETY: `device` is alive.
        command.graphics_pool = unsafe {
            command.device.get_handle().create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(queue_families.graphics),
                None,
            )
        }
        .map_err(|result| Report::new(VulkanCommandError::UnexpectedResult(result)))?;

        // SAFETY: `device` is alive.
        command.transfer_pool = unsafe {
            command.device.get_handle().create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(queue_families.transfer),
                None,
            )
        }
        .map_err(|result| Report::new(VulkanCommandError::UnexpectedResult(result)))?;

        // SAFETY: `device` is alive.
        command.compute_pool = unsafe {
            command.device.get_handle().create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(queue_families.compute),
                None,
            )
        }
        .map_err(|result| Report::new(VulkanCommandError::UnexpectedResult(result)))?;

        // SAFETY: `device` is alive.
        command.graphics_command_buffers = unsafe {
            command.device.get_handle().allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(command.graphics_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }
        .map_err(|result| Report::new(VulkanCommandError::UnexpectedResult(result)))?;

        Ok(command)
    }
}
