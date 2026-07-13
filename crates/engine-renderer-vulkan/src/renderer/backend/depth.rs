use std::sync::Arc;

use ash::vk;
use thiserror::Error;
use vk_mem::{Alloc, Allocation, AllocationCreateFlags, AllocationCreateInfo, MemoryUsage};

use crate::renderer::backend::{
    allocator::VulkanAllocator,
    call_error::VulkanCallError,
    device::{VulkanDevice, VulkanLogicalDevice},
};

pub(super) const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;

#[derive(Debug, Error)]
pub(super) enum VulkanDepthError {
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),
}

pub(super) struct DepthAttachment {
    allocator: Arc<vk_mem::Allocator>,
    device: Arc<VulkanLogicalDevice>,
    image: vk::Image,
    view: vk::ImageView,
    allocation: Allocation,
    extent: vk::Extent2D,
    layout: vk::ImageLayout,
}

impl std::fmt::Debug for DepthAttachment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DepthAttachment")
            .field("image", &self.image)
            .field("view", &self.view)
            .field("extent", &self.extent)
            .field("layout", &self.layout)
            .finish_non_exhaustive()
    }
}

impl Drop for DepthAttachment {
    fn drop(&mut self) {
        // SAFETY: `self.view`, `self.image`, and `self.allocation` were
        // created through these device/allocator handles and are destroyed
        // exactly once here.
        unsafe {
            self.device.handle().destroy_image_view(self.view, None);
            self.allocator.destroy_image(self.image, &mut self.allocation);
        }
    }
}

impl DepthAttachment {
    pub(super) fn new(
        allocator: &VulkanAllocator,
        device: &VulkanDevice,
        extent: vk::Extent2D,
    ) -> core::result::Result<Self, VulkanDepthError> {
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(DEPTH_FORMAT)
            .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        let allocation_info = AllocationCreateInfo {
            usage: MemoryUsage::AutoPreferDevice,
            flags: AllocationCreateFlags::empty(),
            ..Default::default()
        };
        let allocator_handle = allocator.handle();

        // SAFETY: `allocator_handle` and `image_info` are valid for the
        // duration of the call. VMA creates and binds the allocation before
        // returning.
        let (image, allocation) = vk_try!("create depth image", unsafe {
            allocator_handle.create_image(&image_info, &allocation_info)
        });

        #[cfg(debug_assertions)]
        vk_try!("name depth image", device.logical().set_name(c"depth image", image));

        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(DEPTH_FORMAT)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::DEPTH,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        // SAFETY: `image` is a live 2D depth image compatible with
        // `view_info`.
        let view = vk_try!("create depth image view", unsafe {
            device.logical().handle().create_image_view(&view_info, None)
        });

        #[cfg(debug_assertions)]
        vk_try!("name depth image view", device.logical().set_name(c"depth image view", view));

        Ok(Self {
            allocator: allocator_handle,
            device: device.logical().clone(),
            image,
            view,
            allocation,
            extent,
            layout: vk::ImageLayout::UNDEFINED,
        })
    }

    pub(super) fn view(&self) -> vk::ImageView {
        self.view
    }

    pub(super) fn transition_for_render(&mut self, command_buffer: vk::CommandBuffer) {
        if self.layout == vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL {
            return;
        }

        let barrier = vk::ImageMemoryBarrier2::default()
            .src_stage_mask(vk::PipelineStageFlags2::NONE)
            .src_access_mask(vk::AccessFlags2::NONE)
            .dst_stage_mask(vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS)
            .dst_access_mask(vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .old_layout(self.layout)
            .new_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(self.image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::DEPTH,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let barriers = [barrier];
        let dependency_info = vk::DependencyInfo::default().image_memory_barriers(&barriers);

        // SAFETY: `command_buffer` is recording, and the barrier references
        // this live depth image.
        unsafe {
            self.device.handle().cmd_pipeline_barrier2(command_buffer, &dependency_info);
        }

        self.layout = vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL;
    }
}
