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
    compute_command_buffer: ash::vk::CommandBuffer,
    render_command_buffers: [ash::vk::CommandBuffer; super::FRAMES_IN_FLIGHT],
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
            compute_command_buffer: CommandBuffer::default(),
            render_command_buffers: [CommandBuffer::default(); super::FRAMES_IN_FLIGHT],
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
        let mut compute_command_buffers = vk_try!("allocate compute command buffers", unsafe {
            command.device.handle().allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(command.compute_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        });

        command.compute_command_buffer = compute_command_buffers
            .pop()
            .ok_or(VulkanCommandError::NoCommandBufferReturned)?;

        #[cfg(debug_assertions)]
        vk_try!(
            "name compute command buffer",
            command
                .device
                .set_name(c"compute command buffer", command.compute_command_buffer),
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

        // SAFETY: `device` is alive.
        let render_command_buffers = vk_try!("allocate render command buffers", unsafe {
            command.device.handle().allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(command.graphics_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(u32::try_from(super::FRAMES_IN_FLIGHT).unwrap_or(0)),
            )
        });

        command.render_command_buffers = render_command_buffers
            .try_into()
            .map_err(|_| VulkanCommandError::NoCommandBufferReturned)?;

        #[cfg(debug_assertions)]
        for (index, buffer) in command.render_command_buffers.iter().enumerate() {
            if let Ok(name) = std::ffi::CString::new(format!("render command buffer {index}")) {
                vk_try!(
                    "name render command buffer",
                    command.device.set_name(name.as_c_str(), *buffer),
                );
            }
        }

        Ok(command)
    }

    pub(super) fn render_command_buffer(&self, frame_index: usize) -> vk::CommandBuffer {
        self.render_command_buffers[frame_index % super::FRAMES_IN_FLIGHT]
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
            single_mip_subresource_range(0),
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
            single_mip_subresource_range(0),
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

    /// Uploads `src` into mip level 0 of `dst`, then generates the
    /// remaining `mip_levels - 1` levels by repeatedly downsampling
    /// with linear-filtered `vkCmdBlitImage` calls (the tutorial's
    /// "Generating Mipmaps" chapter). Every level ends in
    /// `SHADER_READ_ONLY_OPTIMAL`.
    pub(super) fn upload_texture_with_mipmaps(
        &self,
        queue: vk::Queue,
        src: vk::Buffer,
        dst: vk::Image,
        extent: vk::Extent3D,
        mip_levels: u32,
    ) -> core::result::Result<(), VulkanCommandError> {
        let command_buffer = self.graphics_command_buffer;
        let device = self.device.handle();

        // SAFETY: The command buffer was allocated from a pool created with
        // RESET_COMMAND_BUFFER.
        vk_try!("reset graphics command buffer for mipmapped image upload", unsafe {
            device.reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
        });

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        // SAFETY: `command_buffer` is reset and not pending execution.
        vk_try!("begin graphics command buffer for mipmapped image upload", unsafe {
            device.begin_command_buffer(command_buffer, &begin_info)
        });

        transition_image_layout(
            device,
            command_buffer,
            dst,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::PipelineStageFlags2::NONE,
            vk::AccessFlags2::NONE,
            vk::PipelineStageFlags2::TRANSFER,
            vk::AccessFlags2::TRANSFER_WRITE,
            vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: mip_levels,
                base_array_layer: 0,
                layer_count: 1,
            },
        );

        let regions = [vk::BufferImageCopy::default()
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_extent(extent)];

        // SAFETY: Source buffer and destination image are live. Mip level 0
        // is in TRANSFER_DST_OPTIMAL layout.
        unsafe {
            device.cmd_copy_buffer_to_image(
                command_buffer,
                src,
                dst,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &regions,
            );
        }

        let mut mip_width = i32::try_from(extent.width).unwrap_or(i32::MAX).max(1);
        let mut mip_height = i32::try_from(extent.height).unwrap_or(i32::MAX).max(1);

        for level in 1..mip_levels {
            // The previous level was written as a blit destination (or, for
            // level 1, as the initial buffer-to-image copy destination) and
            // now becomes this blit's source.
            transition_image_layout(
                device,
                command_buffer,
                dst,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::PipelineStageFlags2::TRANSFER,
                vk::AccessFlags2::TRANSFER_WRITE,
                vk::PipelineStageFlags2::TRANSFER,
                vk::AccessFlags2::TRANSFER_READ,
                single_mip_subresource_range(level - 1),
            );

            let next_width = (mip_width / 2).max(1);
            let next_height = (mip_height / 2).max(1);

            let blit = vk::ImageBlit::default()
                .src_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: level - 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .src_offsets([
                    vk::Offset3D::default(),
                    vk::Offset3D { x: mip_width, y: mip_height, z: 1 },
                ])
                .dst_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: level,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .dst_offsets([
                    vk::Offset3D::default(),
                    vk::Offset3D { x: next_width, y: next_height, z: 1 },
                ]);

            let regions = [blit];

            // SAFETY: `dst` is live; level `level - 1` is
            // TRANSFER_SRC_OPTIMAL and level `level` is TRANSFER_DST_OPTIMAL
            // (inherited from the upload-wide transition above).
            unsafe {
                device.cmd_blit_image(
                    command_buffer,
                    dst,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    dst,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &regions,
                    vk::Filter::LINEAR,
                );
            }

            transition_image_layout(
                device,
                command_buffer,
                dst,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::PipelineStageFlags2::TRANSFER,
                vk::AccessFlags2::TRANSFER_READ,
                vk::PipelineStageFlags2::FRAGMENT_SHADER,
                vk::AccessFlags2::SHADER_SAMPLED_READ,
                single_mip_subresource_range(level - 1),
            );

            mip_width = next_width;
            mip_height = next_height;
        }

        // The last level was only ever a blit destination and never became
        // a blit source, so it still needs its own transition out of
        // TRANSFER_DST_OPTIMAL.
        transition_image_layout(
            device,
            command_buffer,
            dst,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            vk::PipelineStageFlags2::TRANSFER,
            vk::AccessFlags2::TRANSFER_WRITE,
            vk::PipelineStageFlags2::FRAGMENT_SHADER,
            vk::AccessFlags2::SHADER_SAMPLED_READ,
            single_mip_subresource_range(mip_levels - 1),
        );

        // SAFETY: Recording was begun above and all commands have been
        // emitted.
        vk_try!("end graphics command buffer for mipmapped image upload", unsafe {
            device.end_command_buffer(command_buffer)
        });

        let command_buffers = [command_buffer];
        let submit_infos = [vk::SubmitInfo::default().command_buffers(&command_buffers)];

        // SAFETY: `queue` belongs to the same device as the command buffer.
        // Waiting for queue idle makes this one-shot upload complete before
        // staging resources are dropped.
        unsafe {
            vk_try!(
                "submit mipmapped image upload",
                device.queue_submit(queue, &submit_infos, vk::Fence::null()),
            );
            vk_try!("wait for mipmapped image upload", device.queue_wait_idle(queue));
        }

        Ok(())
    }

    /// Dispatches `pipeline` on the compute queue with
    /// `push_constants` bound to `pipeline_layout`, then copies
    /// `size` bytes from `src` to `dst` once the dispatch's
    /// writes to `src` are visible (`vkCmdPipelineBarrier2` from
    /// `COMPUTE_SHADER`/`SHADER_WRITE` to
    /// `COPY`/`TRANSFER_READ`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn dispatch_compute_and_readback(
        &self,
        queue: vk::Queue,
        pipeline: vk::Pipeline,
        pipeline_layout: vk::PipelineLayout,
        push_constants: &[u8],
        group_counts: [u32; 3],
        src: vk::Buffer,
        dst: vk::Buffer,
        size: vk::DeviceSize,
    ) -> core::result::Result<(), VulkanCommandError> {
        let command_buffer = self.compute_command_buffer;
        let device = self.device.handle();

        // SAFETY: The command buffer was allocated from a pool created with
        // RESET_COMMAND_BUFFER.
        vk_try!("reset compute command buffer", unsafe {
            device.reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
        });

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        // SAFETY: `command_buffer` is reset and not pending execution.
        vk_try!("begin compute command buffer", unsafe {
            device.begin_command_buffer(command_buffer, &begin_info)
        });

        // SAFETY: `command_buffer` is recording, and `pipeline` and
        // `pipeline_layout` are live for the duration of this call.
        unsafe {
            device.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::COMPUTE, pipeline);

            device.cmd_push_constants(
                command_buffer,
                pipeline_layout,
                vk::ShaderStageFlags::COMPUTE,
                0,
                push_constants,
            );

            device.cmd_dispatch(command_buffer, group_counts[0], group_counts[1], group_counts[2]);
        }

        let barriers = [vk::BufferMemoryBarrier2::default()
            .src_stage_mask(vk::PipelineStageFlags2::COMPUTE_SHADER)
            .src_access_mask(vk::AccessFlags2::SHADER_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags2::COPY)
            .dst_access_mask(vk::AccessFlags2::TRANSFER_READ)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .buffer(src)
            .offset(0)
            .size(size)];

        let dependency_info = vk::DependencyInfo::default().buffer_memory_barriers(&barriers);

        // SAFETY: `command_buffer` is recording, and `src` is a live buffer
        // written by the dispatch above.
        unsafe {
            device.cmd_pipeline_barrier2(command_buffer, &dependency_info);
        }

        let regions = [vk::BufferCopy::default().size(size)];

        // SAFETY: `src` and `dst` are live buffers, and the copy region
        // stays within their caller-provided sizes.
        unsafe {
            device.cmd_copy_buffer(command_buffer, src, dst, &regions);
        }

        // SAFETY: Recording was begun above and all commands have been
        // emitted.
        vk_try!("end compute command buffer", unsafe {
            device.end_command_buffer(command_buffer)
        });

        let command_buffers = [command_buffer];
        let submit_infos = [vk::SubmitInfo::default().command_buffers(&command_buffers)];

        // SAFETY: `queue` belongs to the same device and queue family as
        // the compute command pool. Waiting for queue idle makes this
        // one-shot dispatch complete before its buffers are read or
        // dropped.
        unsafe {
            vk_try!(
                "submit compute dispatch",
                device.queue_submit(queue, &submit_infos, vk::Fence::null()),
            );
            vk_try!("wait for compute dispatch", device.queue_wait_idle(queue));
        }

        Ok(())
    }
}

fn single_mip_subresource_range(mip_level: u32) -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        base_mip_level: mip_level,
        level_count: 1,
        base_array_layer: 0,
        layer_count: 1,
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
    subresource_range: vk::ImageSubresourceRange,
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
        .subresource_range(subresource_range);

    let barriers = [barrier];
    let dependency_info = vk::DependencyInfo::default().image_memory_barriers(&barriers);

    // SAFETY: `command_buffer` is recording, and the barrier references a
    // live image owned by the renderer.
    unsafe {
        device.cmd_pipeline_barrier2(command_buffer, &dependency_info);
    }
}
