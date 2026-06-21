use ash::vk;
use common::logging::macros::*;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use thiserror::Error;

use super::VulkanBackend;
use crate::renderer::backend::instance;

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub enum VulkanSurfaceError {
    /// Failed to create the vulkan instance
    #[error("vulkan result has an error value: [{0:?}] {0}")]
    UnexpectedResult(ash::vk::Result),

    /// Failed to load the vulkan entry
    #[error("transparent")]
    DisplayHandleError,
}

pub(super) struct VulkanSurface {
    raw: ash::vk::SurfaceKHR,
    loader: ash::khr::surface::Instance,
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
        let raw = unsafe { ash_window::create_surface(entry, instance.get(), display_handle, window_handle, None) }.map_err(VulkanSurfaceError::UnexpectedResult)?;

        let loader = ash::khr::surface::Instance::new(entry, instance.get());

        let surface = VulkanSurface { raw, loader };

        // FIXME: Intentional for debugging purposes.
        Err(VulkanSurfaceError::UnexpectedResult(vk::Result::ERROR_OUT_OF_POOL_MEMORY))?;

        trace!("surface initialized");

        Ok(surface)
    }
}
