#[cfg(debug_assertions)]
use std::ffi::CString;
use std::sync::Arc;

use ash::vk;
use common::logging::macros::*;
use thiserror::Error;

use crate::renderer::backend::{
    call_error::VulkanCallError,
    device::{self},
    instance::VulkanInstance,
    surface::{SurfaceConfig, VulkanSurface},
};

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanSwapchainError {
    /// Vulkan API call returned an error value.
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),

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
            unsafe { self.device.handle().destroy_image_view(image_view, None) };
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

    pub(super) fn loader(&self) -> &ash::khr::swapchain::Device {
        &self.loader
    }

    pub(super) fn image(&self, index: u32) -> Option<vk::Image> {
        usize::try_from(index).ok().and_then(|index| self.present_images.get(index)).copied()
    }

    pub(super) fn image_view(&self, index: u32) -> Option<vk::ImageView> {
        usize::try_from(index).ok().and_then(|index| self.present_image_views.get(index)).copied()
    }

    pub(super) fn image_count(&self) -> usize {
        self.present_images.len()
    }
}

impl VulkanSwapchain {
    pub(super) fn new(
        instance: &VulkanInstance,
        device: Arc<device::VulkanLogicalDevice>,
        surface: &VulkanSurface,
        surface_config: &SurfaceConfig,
    ) -> core::result::Result<Self, VulkanSwapchainError> {
        Self::new_replacing(instance, device, surface, surface_config, vk::SwapchainKHR::null())
    }

    pub(super) fn new_replacing(
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
        old_swapchain: vk::SwapchainKHR,
    ) -> core::result::Result<Self, VulkanSwapchainError> {
        let loader = ash::khr::swapchain::Device::new(instance.handle(), device.handle());

        let desired_image_count = choose_image_count(capabilities);

        let image_usage = vk::ImageUsageFlags::COLOR_ATTACHMENT;
        if !capabilities.supported_usage_flags.contains(image_usage) {
            return Err(VulkanSwapchainError::UnsupportedColorAttachment);
        }

        let composite_alpha = choose_composite_alpha(capabilities)?;

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface.handle())
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
            .image_array_layers(1)
            .old_swapchain(old_swapchain);

        // SAFETY: `swapchain_loader` is alive, and `swapchain_create_info`
        // references the surface constructed under the same instance.
        let handle = vk_try!("create swapchain", unsafe {
            loader.create_swapchain(&swapchain_create_info, None)
        });

        let mut sc = Self {
            loader,
            device,
            handle,
            present_images: Vec::default(),
            present_image_views: Vec::default(),
        };

        #[cfg(debug_assertions)]
        vk_try!("name swapchain", sc.device.set_name(c"Swapchain", sc.handle));

        // SAFETY: `handle` was constructed through `swapchain_loader`.
        sc.present_images =
            vk_try!("get swapchain images", unsafe { sc.loader.get_swapchain_images(handle) });

        #[cfg(debug_assertions)]
        for (index, image) in sc.present_images.iter().copied().enumerate() {
            if let Ok(name) = CString::new(format!("Swapchain Image {index}")) {
                vk_try!("name swapchain image", sc.device.set_name(name.as_c_str(), image));
            }
        }

        sc.present_image_views = Vec::with_capacity(sc.present_images.len());

        for (index, image) in sc.present_images.iter().copied().enumerate() {
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
            let image_view = vk_try!("create swapchain image view", unsafe {
                sc.device.handle().create_image_view(&create_view_info, None)
            });

            sc.present_image_views.push(image_view);

            #[cfg(debug_assertions)]
            if let Ok(name) = CString::new(format!("Swapchain Image View {index}")) {
                vk_try!(
                    "name swapchain image view",
                    sc.device.set_name(name.as_c_str(), image_view),
                );
            }
        }

        trace!("swapchain initialized");

        Ok(sc)
    }
}

fn choose_image_count(capabilities: &vk::SurfaceCapabilitiesKHR) -> u32 {
    let desired = capabilities.min_image_count + 1;

    if capabilities.max_image_count > 0 && desired > capabilities.max_image_count {
        capabilities.max_image_count
    } else {
        desired
    }
}

fn choose_composite_alpha(
    capabilities: &vk::SurfaceCapabilitiesKHR,
) -> core::result::Result<vk::CompositeAlphaFlagsKHR, VulkanSwapchainError> {
    [
        vk::CompositeAlphaFlagsKHR::OPAQUE,
        vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
        vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
        vk::CompositeAlphaFlagsKHR::INHERIT,
    ]
    .into_iter()
    .find(|mode| capabilities.supported_composite_alpha.contains(*mode))
    .ok_or(VulkanSwapchainError::UnsupportedCompositeAlpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_count_uses_one_more_than_min_when_under_max() {
        let capabilities = vk::SurfaceCapabilitiesKHR {
            min_image_count: 2,
            max_image_count: 4,
            ..Default::default()
        };

        assert_eq!(choose_image_count(&capabilities), 3);
    }

    #[test]
    fn image_count_clamps_to_max_when_needed() {
        let capabilities = vk::SurfaceCapabilitiesKHR {
            min_image_count: 2,
            max_image_count: 2,
            ..Default::default()
        };

        assert_eq!(choose_image_count(&capabilities), 2);
    }

    #[test]
    fn composite_alpha_prefers_opaque() {
        let capabilities = vk::SurfaceCapabilitiesKHR {
            supported_composite_alpha: vk::CompositeAlphaFlagsKHR::INHERIT
                | vk::CompositeAlphaFlagsKHR::OPAQUE,
            ..Default::default()
        };

        assert_eq!(
            choose_composite_alpha(&capabilities).ok(),
            Some(vk::CompositeAlphaFlagsKHR::OPAQUE)
        );
    }
}
