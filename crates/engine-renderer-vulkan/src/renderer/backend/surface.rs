use ash::vk::{
    ColorSpaceKHR, Extent2D, Format, PresentModeKHR, SurfaceCapabilitiesKHR, SurfaceFormatKHR,
    SurfaceKHR,
};
use common::logging::macros::*;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use thiserror::Error;

use super::VulkanBackend;
use crate::renderer::backend::{device, instance};

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub enum VulkanSurfaceError {
    /// Failed to create the vulkan instance
    #[error("vulkan result has an error value: {0}")]
    UnexpectedResult(ash::vk::Result),

    /// Failed to load the vulkan entry
    #[error("transparent")]
    DisplayHandleError,

    /// Failed to retrieve surface formats
    #[error("transparent")]
    NoSurfaceFormats,
}

pub(super) struct VulkanSurface {
    details: Option<SurfaceDetails>,
    raw: SurfaceKHR,
    loader: ash::khr::surface::Instance,
}

pub(super) struct SurfaceDetails {
    capabilities: SurfaceCapabilitiesKHR,
    formats: Vec<SurfaceFormatKHR>,
    present_modes: Vec<PresentModeKHR>,

    extent: Extent2D,
    surface_format: SurfaceFormatKHR,
    present_mode: PresentModeKHR,
}

impl Drop for VulkanSurface {
    fn drop(&mut self) {
        // SAFETY: `self.raw` was created by this loader with no custom allocator,
        // and the surface wrapper destroys it exactly once.
        unsafe {
            self.loader.destroy_surface(self.raw, None);
        }

        trace!("surface destroyed");
    }
}

impl std::fmt::Debug for VulkanSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Use debug names here
        f.debug_struct("<Vulkan Surface>").finish()
    }
}

impl VulkanSurface {
    pub(super) fn get(&self) -> ash::vk::SurfaceKHR {
        self.raw
    }

    pub(super) fn get_loader(&self) -> &ash::khr::surface::Instance {
        &self.loader
    }

    pub(super) fn refresh(
        &mut self,
        device: &device::VulkanDevice,
        window_dimensions: Extent2D,
        vsync: bool,
    ) -> Result<(), VulkanSurfaceError> {
        // SAFETY: `self.raw` is a live surface created from the same instance as
        // `self.loader`, and `device.get_physical()` was selected from that instance.
        let capabilities = unsafe {
            self.loader
                .get_physical_device_surface_capabilities(device.get_physical(), self.raw)
                .map_err(VulkanSurfaceError::UnexpectedResult)?
        };

        // SAFETY: `self.raw` is a live surface created from the same instance as
        // `self.loader`, and `device.get_physical()` was selected from that instance.
        let formats = unsafe {
            self.loader
                .get_physical_device_surface_formats(device.get_physical(), self.raw)
                .map_err(VulkanSurfaceError::UnexpectedResult)?
        };

        // SAFETY: `self.raw` is a live surface created from the same instance as
        // `self.loader`, and `device.get_physical()` was selected from that instance.
        let present_modes = unsafe {
            self.loader
                .get_physical_device_surface_present_modes(device.get_physical(), self.raw)
                .map_err(VulkanSurfaceError::UnexpectedResult)?
        };

        let extent = if capabilities.current_extent.width == u32::MAX {
            window_dimensions
        } else {
            capabilities.current_extent
        };

        let surface_format = formats
            .iter()
            .copied()
            .find(|format| {
                format.format == Format::B8G8R8A8_SRGB
                    && format.color_space == ColorSpaceKHR::SRGB_NONLINEAR
            })
            .or_else(|| formats.first().copied())
            .ok_or(VulkanSurfaceError::NoSurfaceFormats)?;

        let present_mode = (!vsync)
            .then(|| present_modes.iter().copied().find(|mode| *mode == PresentModeKHR::IMMEDIATE))
            .flatten()
            .unwrap_or(PresentModeKHR::FIFO);

        self.details = Some(SurfaceDetails {
            capabilities,
            formats,
            present_modes,
            extent,
            surface_format,
            present_mode,
        });

        Ok(())
    }
}

impl VulkanBackend {
    pub(super) fn create_surface(
        entry: &ash::Entry,
        instance: &instance::VulkanInstance,
        display_handle: RawDisplayHandle,
        window_handle: RawWindowHandle,
    ) -> Result<VulkanSurface, VulkanSurfaceError> {
        // SAFETY: The raw display/window handles come from the live winit window,
        // and `instance` was created with the required platform surface extensions.
        let raw = unsafe {
            ash_window::create_surface(entry, instance.get(), display_handle, window_handle, None)
        }
        .map_err(VulkanSurfaceError::UnexpectedResult)?;

        let loader = ash::khr::surface::Instance::new(entry, instance.get());

        let surface = VulkanSurface { raw, loader, details: None };

        trace!("surface initialized");

        Ok(surface)
    }
}
