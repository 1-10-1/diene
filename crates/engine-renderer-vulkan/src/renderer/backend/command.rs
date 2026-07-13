use std::sync::Arc;

use ash::vk::{self, CommandBuffer, CommandPool, Handle};
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

    /// Command buffer allocation succeeded without returning a
    /// command buffer.
    #[error("graphics command buffer allocation returned no buffers")]
    NoCommandBufferReturned,
}

#[allow(dead_code)]
pub(super) struct VulkanCommand {
    graphics_pool: ash::vk::CommandPool,
    transfer_pool: ash::vk::CommandPool,
    compute_pool: ash::vk::CommandPool,
    graphics_command_buffer: ash::vk::CommandBuffer,
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
            graphics_command_buffer: CommandBuffer::default(),
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
            command.device.set_name(c"graphics command pool", command.graphics_pool),
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
            command.device.set_name(c"transfer command pool", command.transfer_pool),
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
            command.device.set_name(c"compute command pool", command.compute_pool),
        );

        // SAFETY: `device` is alive.
        let mut graphics_command_buffers = vk_try!("allocate graphics command buffers", unsafe {
            command.device.handle().allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(command.graphics_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        });

        command.graphics_command_buffer = graphics_command_buffers
            .pop()
            .ok_or(VulkanCommandError::NoCommandBufferReturned)?;

        #[cfg(debug_assertions)]
        vk_try!(
            "name graphics command buffer",
            command
                .device
                .set_name(c"graphics command buffer", command.graphics_command_buffer),
        );

        Ok(command)
    }

    pub(super) fn graphics_command_buffer(&self) -> vk::CommandBuffer {
        self.graphics_command_buffer
    }

    pub(super) fn copy_buffer(
        &self,
        queue: vk::Queue,
        src: vk::Buffer,
        dst: vk::Buffer,
        size: vk::DeviceSize,
    ) -> core::result::Result<(), VulkanCommandError> {
        let command_buffer = self.graphics_command_buffer;

        // SAFETY: The command buffer was allocated from a pool created with
        // RESET_COMMAND_BUFFER.
        vk_try!("reset graphics command buffer for copy", unsafe {
            self.device
                .handle()
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
        });

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        // SAFETY: `command_buffer` is reset and not pending execution.
        vk_try!("begin graphics command buffer for copy", unsafe {
            self.device.handle().begin_command_buffer(command_buffer, &begin_info)
        });

        let regions = [vk::BufferCopy::default().size(size)];

        // SAFETY: Both buffers are live, and the copy region stays within the
        // caller-provided buffer sizes by construction.
        unsafe {
            self.device.handle().cmd_copy_buffer(command_buffer, src, dst, &regions);
        }

        // SAFETY: Recording was begun above and contains only the copy
        // command.
        vk_try!("end graphics command buffer for copy", unsafe {
            self.device.handle().end_command_buffer(command_buffer)
        });

        let command_buffers = [command_buffer];
        let submit_infos = [vk::SubmitInfo::default().command_buffers(&command_buffers)];

        // SAFETY: `queue` belongs to the same device as the command buffer.
        // Waiting for queue idle makes this one-shot upload complete
        // before staging resources are dropped.
        unsafe {
            vk_try!(
                "submit buffer copy",
                self.device.handle().queue_submit(queue, &submit_infos, vk::Fence::null()),
            );
            vk_try!("wait for buffer copy", self.device.handle().queue_wait_idle(queue));
        }

        Ok(())
    }

    pub(super) fn copy_buffer_to_image(
        &self,
        queue: vk::Queue,
        src: vk::Buffer,
        dst: vk::Image,
        extent: vk::Extent3D,
    ) -> core::result::Result<(), VulkanCommandError> {
        let command_buffer = self.graphics_command_buffer;

        // SAFETY: The command buffer was allocated from a pool created with
        // RESET_COMMAND_BUFFER.
        vk_try!("reset graphics command buffer for image copy", unsafe {
            self.device
                .handle()
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
        });

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        // SAFETY: `command_buffer` is reset and not pending execution.
        vk_try!("begin graphics command buffer for image copy", unsafe {
            self.device.handle().begin_command_buffer(command_buffer, &begin_info)
        });

        transition_image_layout(
            self.device.handle(),
            command_buffer,
            dst,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::PipelineStageFlags2::NONE,
            vk::AccessFlags2::NONE,
            vk::PipelineStageFlags2::TRANSFER,
            vk::AccessFlags2::TRANSFER_WRITE,
        );

        let regions = [vk::BufferImageCopy::default()
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_extent(extent)];

        // SAFETY: Source buffer and destination image are live. The image is
        // in TRANSFER_DST_OPTIMAL layout and the copy covers mip 0 layer 0.
        unsafe {
            self.device.handle().cmd_copy_buffer_to_image(
                command_buffer,
                src,
                dst,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &regions,
            );
        }

        transition_image_layout(
            self.device.handle(),
            command_buffer,
            dst,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::PipelineStageFlags2::TRANSFER,
            vk::AccessFlags2::TRANSFER_WRITE,
            vk::PipelineStageFlags2::FRAGMENT_SHADER,
            vk::AccessFlags2::SHADER_SAMPLED_READ,
        );

        // SAFETY: Recording was begun above and contains only upload
        // commands.
        vk_try!("end graphics command buffer for image copy", unsafe {
            self.device.handle().end_command_buffer(command_buffer)
        });

        let command_buffers = [command_buffer];
        let submit_infos = [vk::SubmitInfo::default().command_buffers(&command_buffers)];

        // SAFETY: `queue` belongs to the same device as the command buffer.
        // Waiting for queue idle makes this one-shot upload complete before
        // staging resources are dropped.
        unsafe {
            vk_try!(
                "submit image copy",
                self.device.handle().queue_submit(queue, &submit_infos, vk::Fence::null()),
            );
            vk_try!("wait for image copy", self.device.handle().queue_wait_idle(queue));
        }

        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn transition_image_layout(
    device: &ash::Device,
    command_buffer: vk::CommandBuffer,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
    src_stage: vk::PipelineStageFlags2,
    src_access: vk::AccessFlags2,
    dst_stage: vk::PipelineStageFlags2,
    dst_access: vk::AccessFlags2,
) {
    let barrier = vk::ImageMemoryBarrier2::default()
        .src_stage_mask(src_stage)
        .src_access_mask(src_access)
        .dst_stage_mask(dst_stage)
        .dst_access_mask(dst_access)
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        });

    let barriers = [barrier];
    let dependency_info = vk::DependencyInfo::default().image_memory_barriers(&barriers);

    // SAFETY: `command_buffer` is recording, and the barrier references a
    // live image owned by the renderer.
    unsafe {
        device.cmd_pipeline_barrier2(command_buffer, &dependency_info);
    }
}
