use ash::vk::{
    ColorSpaceKHR, Extent2D, Format, PresentModeKHR, SurfaceCapabilitiesKHR, SurfaceFormatKHR,
    SurfaceKHR,
};
use common::logging::macros::*;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use thiserror::Error;

use crate::renderer::backend::{call_error::VulkanCallError, instance};

const PREFERRED_FORMAT: SurfaceFormatKHR =
    SurfaceFormatKHR { format: Format::B8G8R8A8_SRGB, color_space: ColorSpaceKHR::SRGB_NONLINEAR };

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanSurfaceError {
    /// Vulkan API call returned an error value.
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),

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
        // SAFETY: `self.handle` was created by this loader with no custom
        // allocator, and the surface wrapper destroys it exactly
        // once.
        unsafe {
            self.loader.destroy_surface(self.handle, None);
        }

        trace!("surface destroyed");
    }
}

impl VulkanSurface {
    pub(super) fn handle(&self) -> ash::vk::SurfaceKHR {
        self.handle
    }

    pub(super) fn loader(&self) -> &ash::khr::surface::Instance {
        &self.loader
    }

    pub(super) fn new(
        entry: &ash::Entry,
        instance: &instance::VulkanInstance,
        display_handle: RawDisplayHandle,
        window_handle: RawWindowHandle,
    ) -> core::result::Result<Self, VulkanSurfaceError> {
        // SAFETY: The raw display/window handles come from the live winit
        // window, and `instance` was created with the required
        // platform surface extensions.
        let handle = vk_try!("create Vulkan surface", unsafe {
            ash_window::create_surface(
                entry,
                instance.handle(),
                display_handle,
                window_handle,
                None,
            )
        });

        let loader = ash::khr::surface::Instance::new(entry, instance.handle());

        let surface = Self { handle, loader };

        trace!("surface initialized");

        Ok(surface)
    }

    pub(super) fn make_config(
        &self,
        physical_device: ash::vk::PhysicalDevice,
        window_dimensions: Extent2D,
        vsync: bool,
    ) -> core::result::Result<SurfaceConfig, VulkanSurfaceError> {
        // SAFETY: `self.handle` is a live surface created from the same
        // instance as `self.loader`, and `physical_device` was
        // selected from that instance.
        let capabilities = vk_try!("query surface capabilities", unsafe {
            self.loader
                .get_physical_device_surface_capabilities(physical_device, self.handle)
        });

        // SAFETY: `self.handle` is a live surface created from the same
        // instance as `self.loader`, and `physical_device` was
        // selected from that instance.
        let formats = vk_try!("query surface formats", unsafe {
            self.loader.get_physical_device_surface_formats(physical_device, self.handle)
        });

        // SAFETY: `self.handle` is a live surface created from the same
        // instance as `self.loader`, and `physical_device` was
        // selected from that instance.
        let present_modes = vk_try!("query surface present modes", unsafe {
            self.loader
                .get_physical_device_surface_present_modes(physical_device, self.handle)
        });

        let extent = choose_extent(&capabilities, window_dimensions);
        let surface_format = choose_surface_format(&formats)?;
        let present_mode = choose_present_mode(&present_modes, vsync)?;

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

fn choose_extent(
    capabilities: &SurfaceCapabilitiesKHR,
    mut window_dimensions: Extent2D,
) -> Extent2D {
    if capabilities.current_extent.width != u32::MAX {
        return capabilities.current_extent;
    }

    window_dimensions.width = window_dimensions
        .width
        .clamp(capabilities.min_image_extent.width, capabilities.max_image_extent.width);
    window_dimensions.height = window_dimensions
        .height
        .clamp(capabilities.min_image_extent.height, capabilities.max_image_extent.height);
    window_dimensions
}

fn choose_surface_format(
    formats: &[SurfaceFormatKHR],
) -> core::result::Result<SurfaceFormatKHR, VulkanSurfaceError> {
    Ok(match formats {
        [] => return Err(VulkanSurfaceError::NoSurfaceFormats),
        [format] if format.format == Format::UNDEFINED => PREFERRED_FORMAT,
        formats => formats
            .iter()
            .copied()
            .find(|format| {
                format.format == PREFERRED_FORMAT.format
                    && format.color_space == PREFERRED_FORMAT.color_space
            })
            .unwrap_or(formats[0]),
    })
}

fn choose_present_mode(
    present_modes: &[PresentModeKHR],
    vsync: bool,
) -> core::result::Result<PresentModeKHR, VulkanSurfaceError> {
    if present_modes.is_empty() {
        return Err(VulkanSurfaceError::NoPresentModes);
    }

    Ok((!vsync)
        .then(|| present_modes.iter().copied().find(|mode| *mode == PresentModeKHR::IMMEDIATE))
        .flatten()
        .unwrap_or(PresentModeKHR::FIFO))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_window_extent_when_surface_has_no_fixed_extent() {
        let capabilities = SurfaceCapabilitiesKHR {
            current_extent: Extent2D { width: u32::MAX, height: u32::MAX },
            min_image_extent: Extent2D { width: 100, height: 200 },
            max_image_extent: Extent2D { width: 400, height: 500 },
            ..Default::default()
        };

        assert_eq!(
            choose_extent(&capabilities, Extent2D { width: 50, height: 600 }),
            Extent2D { width: 100, height: 500 }
        );
    }

    #[test]
    fn prefers_srgb_surface_format_when_available() {
        let fallback = SurfaceFormatKHR {
            format: Format::R8G8B8A8_UNORM,
            color_space: ColorSpaceKHR::SRGB_NONLINEAR,
        };

        assert_eq!(
            choose_surface_format(&[fallback, PREFERRED_FORMAT]).ok(),
            Some(PREFERRED_FORMAT)
        );
    }

    #[test]
    fn disables_vsync_with_immediate_present_when_available() {
        assert_eq!(
            choose_present_mode(&[PresentModeKHR::FIFO, PresentModeKHR::IMMEDIATE], false).ok(),
            Some(PresentModeKHR::IMMEDIATE)
        );
    }
}
