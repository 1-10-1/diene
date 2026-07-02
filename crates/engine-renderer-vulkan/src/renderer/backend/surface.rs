use ash::vk::{
    ColorSpaceKHR, Extent2D, Format, PresentModeKHR, SurfaceCapabilitiesKHR, SurfaceFormatKHR,
    SurfaceKHR,
};
use common::logging::macros::*;
use error_stack::{Report, ResultExt};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use thiserror::Error;

use super::VulkanBackend;
use crate::renderer::backend::instance;

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanSurfaceError {
    /// Vulkan API call returned an error value.
    #[error("vulkan result has an error value: {0}")]
    UnexpectedResult(ash::vk::Result),

    /// Failed to retrieve surface formats
    #[error("no surface formats available")]
    NoSurfaceFormats,

    /// Failed to retrieve surface present modes.
    #[error("no surface present modes available")]
    NoPresentModes,
}

pub(super) struct VulkanSurface {
    handle: SurfaceKHR,
    loader: ash::khr::surface::Instance,
}

#[expect(dead_code)]
pub(super) struct SurfaceConfig {
    pub(super) capabilities: SurfaceCapabilitiesKHR,
    pub(super) formats: Vec<SurfaceFormatKHR>,
    pub(super) present_modes: Vec<PresentModeKHR>,

    pub(super) extent: Extent2D,
    pub(super) surface_format: SurfaceFormatKHR,
    pub(super) present_mode: PresentModeKHR,
}

impl Drop for VulkanSurface {
    fn drop(&mut self) {
        // SAFETY: `self.handle` was created by this loader with no custom allocator,
        // and the surface wrapper destroys it exactly once.
        unsafe {
            self.loader.destroy_surface(self.handle, None);
        }

        trace!("surface destroyed");
    }
}

impl VulkanSurface {
    pub(super) fn get(&self) -> ash::vk::SurfaceKHR {
        self.handle
    }

    pub(super) fn get_loader(&self) -> &ash::khr::surface::Instance {
        &self.loader
    }

    pub(super) fn make_config(
        &mut self,
        physical_device: ash::vk::PhysicalDevice,
        mut window_dimensions: Extent2D,
        vsync: bool,
    ) -> core::result::Result<SurfaceConfig, VulkanSurfaceError> {
        // SAFETY: `self.handle` is a live surface created from the same instance as
        // `self.loader`, and `physical_device` was selected from that instance.
        let capabilities = unsafe {
            self.loader.get_physical_device_surface_capabilities(physical_device, self.handle)
        }
        .map_err(VulkanSurfaceError::UnexpectedResult)?;

        // SAFETY: `self.handle` is a live surface created from the same instance as
        // `self.loader`, and `physical_device` was selected from that instance.
        let formats = unsafe {
            self.loader.get_physical_device_surface_formats(physical_device, self.handle)
        }
        .map_err(VulkanSurfaceError::UnexpectedResult)?;

        // SAFETY: `self.handle` is a live surface created from the same instance as
        // `self.loader`, and `physical_device` was selected from that instance.
        let present_modes = unsafe {
            self.loader.get_physical_device_surface_present_modes(physical_device, self.handle)
        }
        .map_err(VulkanSurfaceError::UnexpectedResult)?;

        let extent = if capabilities.current_extent.width == u32::MAX {
            window_dimensions.width = window_dimensions
                .width
                .clamp(capabilities.min_image_extent.width, capabilities.max_image_extent.width);
            window_dimensions.height = window_dimensions
                .height
                .clamp(capabilities.min_image_extent.height, capabilities.max_image_extent.height);
            window_dimensions
        } else {
            capabilities.current_extent
        };

        let preferred_format = SurfaceFormatKHR {
            format: Format::B8G8R8A8_SRGB,
            color_space: ColorSpaceKHR::SRGB_NONLINEAR,
        };

        let surface_format = match formats.as_slice() {
            [] => return Err(VulkanSurfaceError::NoSurfaceFormats),
            [format] if format.format == Format::UNDEFINED => preferred_format,
            formats => formats
                .iter()
                .copied()
                .find(|format| {
                    format.format == preferred_format.format
                        && format.color_space == preferred_format.color_space
                })
                .unwrap_or(formats[0]),
        };

        if present_modes.is_empty() {
            return Err(VulkanSurfaceError::NoPresentModes);
        }

        let present_mode = (!vsync)
            .then(|| present_modes.iter().copied().find(|mode| *mode == PresentModeKHR::IMMEDIATE))
            .flatten()
            .unwrap_or(PresentModeKHR::FIFO);

        Ok(SurfaceConfig {
            capabilities,
            formats,
            present_modes,
            extent,
            surface_format,
            present_mode,
        })
    }
}

impl VulkanBackend {
    pub(super) fn create_surface(
        entry: &ash::Entry,
        instance: &instance::VulkanInstance,
        display_handle: RawDisplayHandle,
        window_handle: RawWindowHandle,
    ) -> error_stack::Result<VulkanSurface, VulkanSurfaceError> {
        // SAFETY: The raw display/window handles come from the live winit window,
        // and `instance` was created with the required platform surface extensions.
        let handle = unsafe {
            ash_window::create_surface(entry, instance.get(), display_handle, window_handle, None)
        }
        .map_err(|result| Report::new(VulkanSurfaceError::UnexpectedResult(result)))
        .attach_printable("failed to create vulkan surface")?;

        let loader = ash::khr::surface::Instance::new(entry, instance.get());

        let surface = VulkanSurface { handle, loader };

        trace!("surface initialized");

        Ok(surface)
    }
}
