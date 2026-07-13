use std::sync::Arc;

use ash::vk;
use engine_renderer_api::{TextureData, TextureExtent};
use thiserror::Error;
use vk_mem::{Alloc, Allocation, AllocationCreateFlags, AllocationCreateInfo, MemoryUsage};

use crate::renderer::backend::{
    buffer::{VulkanBuffer, VulkanBufferError},
    call_error::VulkanCallError,
    command::{VulkanCommand, VulkanCommandError},
    device::{VulkanDevice, VulkanLogicalDevice},
};

#[derive(Debug, Error)]
pub(super) enum VulkanImageError {
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),

    #[error(transparent)]
    Buffer(#[from] VulkanBufferError),

    #[error(transparent)]
    Command(#[from] VulkanCommandError),
}

pub(super) struct VulkanImage {
    allocator: Arc<vk_mem::Allocator>,
    device: Arc<VulkanLogicalDevice>,
    handle: vk::Image,
    view: vk::ImageView,
    allocation: Allocation,
    extent: TextureExtent,
    format: vk::Format,
}

impl std::fmt::Debug for VulkanImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanImage")
            .field("handle", &self.handle)
            .field("view", &self.view)
            .field("extent", &self.extent)
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
}

impl Drop for VulkanImage {
    fn drop(&mut self) {
        // SAFETY: `self.view`, `self.handle`, and `self.allocation` were
        // created through these device/allocator handles and are destroyed
        // exactly once here.
        unsafe {
            self.device.handle().destroy_image_view(self.view, None);
            self.allocator.destroy_image(self.handle, &mut self.allocation);
        }
    }
}

impl VulkanImage {
    pub(super) fn from_texture_data(
        allocator: Arc<vk_mem::Allocator>,
        command: &VulkanCommand,
        device: &VulkanDevice,
        name: &'static std::ffi::CStr,
        data: &TextureData,
    ) -> core::result::Result<Self, VulkanImageError> {
        let extent = data.extent();
        let format = vk::Format::R8G8B8A8_SRGB;

        let staging_size = vk::DeviceSize::try_from(data.byte_len())
            .map_err(|_| VulkanBufferError::BufferTooLarge { bytes: data.byte_len() })?;

        let mut staging = VulkanBuffer::new(
            device.logical(),
            allocator.clone(),
            c"texture staging buffer",
            staging_size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            MemoryUsage::AutoPreferHost,
            AllocationCreateFlags::HOST_ACCESS_SEQUENTIAL_WRITE,
        )?;

        staging.write_bytes(data.pixels())?;

        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D { width: extent.width, height: extent.height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let allocation_info = AllocationCreateInfo {
            usage: MemoryUsage::AutoPreferDevice,
            flags: AllocationCreateFlags::empty(),
            ..Default::default()
        };

        // SAFETY: `allocator` and `image_info` are valid for the duration of
        // the call. VMA creates and binds the allocation before returning.
        let (handle, allocation) = vk_try!("create texture image", unsafe {
            allocator.create_image(&image_info, &allocation_info)
        });

        #[cfg(debug_assertions)]
        vk_try!("name texture image", device.logical().set_name(name, handle));

        command.copy_buffer_to_image(
            device.graphics_queue(),
            staging.handle(),
            handle,
            vk::Extent3D { width: extent.width, height: extent.height, depth: 1 },
        )?;

        let view_info = vk::ImageViewCreateInfo::default()
            .image(handle)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        // SAFETY: `handle` is a live 2D color image compatible with
        // `view_info`.
        let view = vk_try!("create texture image view", unsafe {
            device.logical().handle().create_image_view(&view_info, None)
        });

        #[cfg(debug_assertions)]
        vk_try!(
            "name texture image view",
            device.logical().set_name(c"texture image view", view)
        );

        Ok(Self {
            allocator,
            device: device.logical().clone(),
            handle,
            view,
            allocation,
            extent,
            format,
        })
    }

    pub(super) fn view(&self) -> vk::ImageView {
        self.view
    }

    pub(super) fn extent(&self) -> TextureExtent {
        self.extent
    }
}
