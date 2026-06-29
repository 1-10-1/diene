use ash::vk;
use common::logging::macros::*;
use error_stack::Report;
use thiserror::Error;

use super::VulkanBackend;
use crate::renderer::backend::{
    device::VulkanDevice,
    instance::VulkanInstance,
    surface::{SurfaceConfig, VulkanSurface},
};

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanSwapchainError {
    /// Vulkan API call returned an error value.
    #[error("vulkan result has an error value: {0}")]
    UnexpectedResult(ash::vk::Result),
}

#[allow(dead_code)]
pub(super) struct VulkanSwapchain {
    loader: ash::khr::swapchain::Device,
    logical: ash::Device,
    handle: vk::SwapchainKHR,
    present_images: Vec<ash::vk::Image>,
    present_image_views: Vec<ash::vk::ImageView>,
}

impl Drop for VulkanSwapchain {
    fn drop(&mut self) {
        for image_view in self.present_image_views.drain(..) {
            // SAFETY: `image_view` was constructed through `self.logical`.
            // This is called exactly once during drop.
            unsafe { self.logical.destroy_image_view(image_view, None) };
        }

        // SAFETY: `self.loader` created `self.handle` during construction.
        // This is called exactly once during drop.
        unsafe { self.loader.destroy_swapchain(self.handle, None) };

        trace!("swapchain destroyed");
    }
}

impl std::fmt::Debug for VulkanSwapchain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Use debug names here
        f.debug_struct("<Vulkan Swapchain>").finish()
    }
}

impl VulkanSwapchain {
    #[allow(dead_code)]
    pub(super) fn get(&self) -> ash::vk::SwapchainKHR {
        self.handle
    }
}

impl VulkanBackend {
    pub(super) fn create_swapchain(
        instance: &VulkanInstance,
        device: &VulkanDevice,
        surface: &VulkanSurface,
        SurfaceConfig {
            capabilities,
            formats: _,
            present_modes: _,
            extent,
            surface_format,
            present_mode,
        }: &SurfaceConfig,
    ) -> error_stack::Result<VulkanSwapchain, VulkanSwapchainError> {
        let loader = ash::khr::swapchain::Device::new(instance.get(), device.get());

        let mut desired_image_count = capabilities.min_image_count + 1;

        if capabilities.max_image_count > 0 && desired_image_count > capabilities.max_image_count {
            desired_image_count = capabilities.max_image_count;
        }

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface.get())
            .min_image_count(desired_image_count)
            .image_color_space(surface_format.color_space)
            .image_format(surface_format.format)
            .image_extent(*extent)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(*present_mode)
            .clipped(true)
            .image_array_layers(1);

        // SAFETY: `swapchain_loader` is alive, and `swapchain_create_info`
        // references the surface constructed under the same instance.
        let handle = unsafe {
            loader
                .create_swapchain(&swapchain_create_info, None)
                .map_err(VulkanSwapchainError::UnexpectedResult)?
        };

        device
            .set_name(&c"Swapchain".to_owned(), handle)
            .map_err(|result| Report::new(VulkanSwapchainError::UnexpectedResult(result)))?;

        // SAFETY: `handle` was constructed through `swapchain_loader`.
        let present_images = unsafe { loader.get_swapchain_images(handle) }
            .map_err(VulkanSwapchainError::UnexpectedResult)?;

        let mut present_image_views: Vec<vk::ImageView> = Vec::with_capacity(present_images.len());

        for image in present_images.iter().copied() {
            let create_view_info = vk::ImageViewCreateInfo::default()
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(surface_format.format)
                .components(vk::ComponentMapping {
                    r: vk::ComponentSwizzle::R,
                    g: vk::ComponentSwizzle::G,
                    b: vk::ComponentSwizzle::B,
                    a: vk::ComponentSwizzle::A,
                })
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image(image);

            // SAFETY: The underlying image was constructed through the same device.
            let image_view = unsafe { device.get().create_image_view(&create_view_info, None) }
                .map_err(VulkanSwapchainError::UnexpectedResult)?;

            present_image_views.push(image_view);
        }

        trace!("swapchain initialized");

        Ok(VulkanSwapchain {
            loader,
            logical: device.get().clone(),
            handle,
            present_images,
            present_image_views,
        })
    }
}
