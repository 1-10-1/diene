#[macro_use]
mod call_error;

mod allocator;
mod command;
mod device;
mod instance;
mod pipeline;
mod shader;
mod surface;
mod swapchain;

use std::rc::Rc;

use ash::vk::Extent2D;
use engine_renderer_api::{RenderExtent, RenderWindow};
use engine_shader::{ShaderCompiler, ShaderCompilerOptions};
use error_stack::{Report, ResultExt};
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
    #[error("instance operation failed")]
    InstanceOperation,

    /// Vulkan surface operation failed.
    #[error("surface operation failed")]
    SurfaceOperation,

    /// Vulkan device operation failed.
    #[error("logical device operation failed")]
    DeviceOperation,

    /// Vulkan swapchain operation failed.
    #[error("swapchain operation failed")]
    SwapchainOperation,

    /// Vulkan allocator operation failed.
    #[error("allocator operation failed")]
    AllocatorOperation,

    /// Vulkan command operation failed.
    #[error("command operation failed")]
    CommandOperation,

    /// Vulkan surface refresh failed.
    #[error("failed to refresh vulkan surface details")]
    RefreshSurface,

    /// Vulkan shader compiler operation failed.
    #[error("shader compiler operation failed")]
    ShaderCompilation,
}

#[allow(dead_code)]
pub(super) struct VulkanBackend {
    // Rust drops fields in declaration order. Keep Vulkan children above their parents.
    command: command::VulkanCommand,
    allocator: allocator::VulkanAllocator,
    swapchain: swapchain::VulkanSwapchain,
    surface_config: surface::SurfaceConfig,
    device: device::VulkanDevice,
    surface: surface::VulkanSurface,
    instance: instance::VulkanInstance,
    entry: ash::Entry,
}

impl VulkanBackend {
    pub(super) fn new(
        rw: &dyn RenderWindow,
        vsync: bool,
    ) -> error_stack::Result<Self, VulkanBackendError> {
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

        let instance = instance::VulkanInstance::new(&entry, display_handle)
            .change_context(VulkanBackendError::InstanceOperation)?;

        let surface = surface::VulkanSurface::new(&entry, &instance, display_handle, window_handle)
            .change_context(VulkanBackendError::SurfaceOperation)?;

        let device = device::VulkanDevice::new(&instance, &surface)
            .change_context(VulkanBackendError::DeviceOperation)?;

        let RenderExtent { width, height } = rw.size();

        let surface_config = surface
            .make_config(device.physical(), Extent2D { width, height }, vsync)
            .change_context(VulkanBackendError::RefreshSurface)?;

        // WARN: The C++ equivalent recreates fences and semaphores in the
        // constructor for some reason.
        // Figure out the reason, and if valid, implement that.
        let swapchain = swapchain::VulkanSwapchain::new(
            &instance,
            device.logical().clone(),
            &surface,
            &surface_config,
        )
        .change_context(VulkanBackendError::SwapchainOperation)?;

        let allocator =
            allocator::VulkanAllocator::new(&instance, device.logical(), device.physical())
                .change_context(VulkanBackendError::AllocatorOperation)?;

        let command =
            command::VulkanCommand::new(device.logical().clone(), device.queue_families())
                .change_context(VulkanBackendError::CommandOperation)?;

        let compiler = Rc::new(
            ShaderCompiler::with_options(
                ShaderCompilerOptions::default()
                    .with_search_path("shaders")
                    .with_spirv_profile("spirv_1_5")
                    .with_assembly_output_dir("shaders"),
            )
            .change_context(VulkanBackendError::ShaderCompilation)?,
        );

        let shaders = shader::VulkanShaderManager::new(device.logical().clone(), compiler.clone());

        let compiled_main_shader = compiler
            .compile(
                "main",
                [
                    engine_shader::ShaderEntrypoint::new(
                        "fragMain",
                        engine_shader::ShaderStage::Fragment,
                    ),
                    engine_shader::ShaderEntrypoint::new(
                        "vertMain",
                        engine_shader::ShaderStage::Vertex,
                    ),
                ],
            )
            .change_context(VulkanBackendError::ShaderCompilation)?;

        let vk_main_shader = shaders
            .create_shader(&compiled_main_shader)
            .change_context(VulkanBackendError::ShaderCompilation)?;

        let pipe = pipeline::VulkanGraphicsPipeline::builder().with_shaders([&vk_main_shader]);

        Ok(Self { command, allocator, swapchain, surface_config, device, surface, instance, entry })
    }
}
