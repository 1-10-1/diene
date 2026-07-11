use std::sync::Arc;

use ash::vk::{self, CommandPool, Handle};
use thiserror::Error;

use crate::renderer::backend::{
    call_error::VulkanCallError,
    device::{self, QueueFamilyIndices},
};

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanCommandError {
    /// Vulkan API call returned an error value.
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),
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
                self.device.handle().destroy_command_pool(self.graphics_pool, None);
                self.graphics_pool = CommandPool::null();
            }

            if !self.transfer_pool.is_null() {
                self.device.handle().destroy_command_pool(self.transfer_pool, None);
                self.transfer_pool = CommandPool::null();
            }

            if !self.compute_pool.is_null() {
                self.device.handle().destroy_command_pool(self.compute_pool, None);
                self.compute_pool = CommandPool::null();
            }
        }
    }
}

impl VulkanCommand {
    pub(super) fn new(
        device: Arc<device::VulkanLogicalDevice>,
        queue_families: &QueueFamilyIndices,
    ) -> core::result::Result<Self, VulkanCommandError> {
        let mut command = Self {
            graphics_pool: CommandPool::default(),
            transfer_pool: CommandPool::default(),
            compute_pool: CommandPool::default(),
            graphics_command_buffers: Vec::default(),
            device,
        };

        // SAFETY: `device` is alive.
        command.graphics_pool = vk_try!("create graphics command pool", unsafe {
            command.device.handle().create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(queue_families.graphics),
                None,
            )
        });

        #[cfg(debug_assertions)]
        vk_try!(
            "name graphics command pool",
            command.device.set_name(c"Graphics Command Pool", command.graphics_pool),
        );

        // SAFETY: `device` is alive.
        command.transfer_pool = vk_try!("create transfer command pool", unsafe {
            command.device.handle().create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(queue_families.transfer),
                None,
            )
        });

        #[cfg(debug_assertions)]
        vk_try!(
            "name transfer command pool",
            command.device.set_name(c"Transfer Command Pool", command.transfer_pool),
        );

        // SAFETY: `device` is alive.
        command.compute_pool = vk_try!("create compute command pool", unsafe {
            command.device.handle().create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                    .queue_family_index(queue_families.compute),
                None,
            )
        });

        #[cfg(debug_assertions)]
        vk_try!(
            "name compute command pool",
            command.device.set_name(c"Compute Command Pool", command.compute_pool),
        );

        // SAFETY: `device` is alive.
        command.graphics_command_buffers = vk_try!("allocate graphics command buffers", unsafe {
            command.device.handle().allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(command.graphics_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        });

        #[cfg(debug_assertions)]
        for command_buffer in command.graphics_command_buffers.iter().copied() {
            vk_try!(
                "name graphics command buffer",
                command.device.set_name(c"Graphics Command Buffer", command_buffer),
            );
        }

        Ok(command)
    }
}
