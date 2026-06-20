mod device;
mod instance;
mod surface;

use engine_renderer_api::{HandleError, RenderWindow};
use thiserror::Error;

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub enum VulkanBackendError {
    /// Failed to create the vulkan instance
    /// FIXME: The backend should not propagate low-level vulkan results to the
    /// frontend.
    #[error("vulkan result has an error value: [{0:?}] {0}")]
    UnexpectedResult(ash::vk::Result),

    /// Failed to load the vulkan entry
    #[error("entry load failed")]
    EntryLoadFailure,

    /// Failed to load the vulkan entry
    #[error("display handle error: {0}")]
    DisplayHandleError(#[from] HandleError),

    /// Vulkan instance operation failed.
    #[error("vulkan instance error: {0}")]
    Instance(#[from] instance::VulkanInstanceError),

    /// Vulkan surface operation failed.
    #[error("vulkan surface error: {0}")]
    Surface(#[from] surface::VulkanSurfaceError),

    /// Vulkan device operation failed.
    #[error("vulkan device error: {0}")]
    Device(#[from] device::VulkanDeviceError),
}

pub(super) struct VulkanBackend {
    #[allow(dead_code)]
    device: device::VulkanDevice,

    #[allow(dead_code)]
    surface: surface::VulkanSurface,

    #[allow(dead_code)]
    instance: instance::VulkanInstance,

    // Held for RAII and loader lifetime; Vulkan objects are created from this entry.
    #[allow(dead_code)]
    entry: ash::Entry,
}

impl std::fmt::Debug for VulkanBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Use debug names here
        f.debug_struct("<Vulkan Backend>").field("instance", &self.instance).finish_non_exhaustive()
    }
}

impl VulkanBackend {
    pub(super) fn new(rw: &dyn RenderWindow) -> Result<Self, VulkanBackendError> {
        let display_handle = rw.display_handle()?.as_raw();
        let window_handle = rw.window_handle()?.as_raw();

        // SAFETY: Must outlive every other object spawned from it.
        let entry = unsafe { ash::Entry::load() }.map_err(|_| VulkanBackendError::EntryLoadFailure)?;

        let instance = Self::create_instance(&entry, display_handle)?;

        let surface = Self::create_surface(&entry, &instance, display_handle, window_handle)?;

        let device = Self::create_device(&entry, &instance, &surface)?;

        Ok(Self { device, surface, instance, entry })
    }
}
