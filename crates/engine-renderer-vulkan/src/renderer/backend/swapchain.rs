use std::sync::Arc;

use ash::vk;
use common::logging::macros::*;
use thiserror::Error;

use super::VulkanBackend;
use crate::renderer::backend::{
    device::{self},
    instance::VulkanInstance,
    surface::{SurfaceConfig, VulkanSurface},
};

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanSwapchainError {
    /// Vulkan API call returned an error value.
    #[error("vulkan result has an error value: {0}")]
    UnexpectedResult(ash::vk::Result),

    /// Surface does not support presenting color attachment images.
    #[error("surface does not support color attachment swapchain images")]
    UnsupportedColorAttachment,

    /// Surface does not support a known composite alpha mode.
    #[error("surface does not support a known composite alpha mode")]
    UnsupportedCompositeAlpha,
}

#[allow(dead_code)]
pub(super) struct VulkanSwapchain {
    handle: vk::SwapchainKHR,
    present_images: Vec<ash::vk::Image>,
    present_image_views: Vec<ash::vk::ImageView>,
    device: Arc<device::VulkanLogicalDevice>,
    loader: ash::khr::swapchain::Device,
}

impl Drop for VulkanSwapchain {
    fn drop(&mut self) {
        for image_view in self.present_image_views.drain(..) {
            // SAFETY: `image_view` was constructed through `self.device`.
            // This is called exactly once during drop.
            unsafe { self.device.get_handle().destroy_image_view(image_view, None) };
        }

        // SAFETY: `self.loader` created `self.handle` during construction.
        // This is called exactly once during drop.
        unsafe { self.loader.destroy_swapchain(self.handle, None) };

        trace!("swapchain destroyed");
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
        device: Arc<device::VulkanLogicalDevice>,
        surface: &VulkanSurface,
        SurfaceConfig {
            capabilities,
            formats: _,
            present_modes: _,
            extent,
            surface_format,
            present_mode,
        }: &SurfaceConfig,
    ) -> core::result::Result<VulkanSwapchain, VulkanSwapchainError> {
        let loader = ash::khr::swapchain::Device::new(instance.get(), device.get_handle());

        let mut desired_image_count = capabilities.min_image_count + 1;

        if capabilities.max_image_count > 0 && desired_image_count > capabilities.max_image_count {
            desired_image_count = capabilities.max_image_count;
        }

        let image_usage = vk::ImageUsageFlags::COLOR_ATTACHMENT;
        if !capabilities.supported_usage_flags.contains(image_usage) {
            return Err(VulkanSwapchainError::UnsupportedColorAttachment);
        }

        let composite_alpha = [
            vk::CompositeAlphaFlagsKHR::OPAQUE,
            vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
            vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
            vk::CompositeAlphaFlagsKHR::INHERIT,
        ]
        .into_iter()
        .find(|mode| capabilities.supported_composite_alpha.contains(*mode))
        .ok_or(VulkanSwapchainError::UnsupportedCompositeAlpha)?;

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface.get())
            .min_image_count(desired_image_count)
            .image_color_space(surface_format.color_space)
            .image_format(surface_format.format)
            .image_extent(*extent)
            .image_usage(image_usage)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(composite_alpha)
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

        let mut sc = VulkanSwapchain {
            loader,
            device,
            handle,
            present_images: Vec::default(),
            present_image_views: Vec::default(),
        };

        // SAFETY: `handle` was constructed through `swapchain_loader`.
        sc.present_images = unsafe { sc.loader.get_swapchain_images(handle) }
            .map_err(VulkanSwapchainError::UnexpectedResult)?;

        sc.present_image_views = Vec::with_capacity(sc.present_images.len());

        for image in sc.present_images.iter().copied() {
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
            let image_view =
                unsafe { sc.device.get_handle().create_image_view(&create_view_info, None) }
                    .map_err(VulkanSwapchainError::UnexpectedResult)?;

            sc.present_image_views.push(image_view);
        }

        trace!("swapchain initialized");

        Ok(sc)
    }
}
