mod device;
mod instance;
mod surface;
mod swapchain;

use ash::vk::Extent2D;
use engine_renderer_api::{RenderExtent, RenderWindow};
use error_stack::{Report, Result, ResultExt};
use thiserror::Error;

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanBackendError {
    /// Failed to load the vulkan entry
    #[error("failed to load the vulkan entry")]
    EntryLoadFailure,

    /// Failed to get the display handle.
    #[error("failed to get display handle")]
    DisplayHandle,

    /// Failed to get the window handle.
    #[error("failed to get window handle")]
    WindowHandle,

    /// Vulkan instance operation failed.
    #[error("failed to create vulkan instance")]
    InstanceOperation,

    /// Vulkan surface operation failed.
    #[error("failed to create vulkan surface")]
    SurfaceOperation,

    /// Vulkan device operation failed.
    #[error("failed to create vulkan logical device")]
    DeviceOperation,

    /// Vulkan swapchain operation failed.
    #[error("failed to create vulkan swapchain")]
    SwapchainOperation,

    /// Vulkan surface refresh failed.
    #[error("failed to refresh vulkan surface details")]
    RefreshSurface,
}

pub(super) struct VulkanBackend {
    #[allow(dead_code)]
    swapchain: swapchain::VulkanSwapchain,

    #[allow(dead_code)]
    surface_config: surface::SurfaceConfig,

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
    pub(super) fn new(rw: &dyn RenderWindow, vsync: bool) -> Result<Self, VulkanBackendError> {
        let display_handle = rw
            .display_handle()
            .map_err(|error| {
                Report::new(VulkanBackendError::DisplayHandle).attach_printable(error.to_string())
            })?
            .as_raw();
        let window_handle = rw
            .window_handle()
            .map_err(|error| {
                Report::new(VulkanBackendError::WindowHandle).attach_printable(error.to_string())
            })?
            .as_raw();

        // SAFETY: Loading the Vulkan entry only performs dynamic symbol lookup;
        // the owned entry is stored in the backend and outlives all objects
        // created from it.
        let entry = unsafe { ash::Entry::load() }.map_err(|err| {
            Report::new(VulkanBackendError::EntryLoadFailure).attach_printable(err.to_string())
        })?;

        let instance = Self::create_instance(&entry, display_handle)
            .change_context(VulkanBackendError::InstanceOperation)?;

        let mut surface = Self::create_surface(&entry, &instance, display_handle, window_handle)
            .change_context(VulkanBackendError::SurfaceOperation)?;

        let device = Self::create_device(&instance, &surface)
            .change_context(VulkanBackendError::DeviceOperation)?;

        let RenderExtent { width, height } = rw.size();

        let surface_config = surface
            .make_config(&device, Extent2D { width, height }, vsync)
            .change_context(VulkanBackendError::RefreshSurface)?;

        // WARN: The C++ equivalent recreates fences and semaphores in the
        // constructor for some reason.
        // Figure out the reason, and if valid, implement that.
        let swapchain = Self::create_swapchain(&instance, &device, &surface, &surface_config)
            .change_context(VulkanBackendError::SwapchainOperation)?;

        // FIXME: vk_mem uses 0.38 :( what to do? fork maybe?
        // vk_mem::Allocator::new(AllocatorCreateInfo::new(
        //     instance.get(),
        //     device.get(),
        //     device.get_physical(),
        // ));

        Ok(Self { swapchain, surface_config, device, surface, instance, entry })
    }
}
