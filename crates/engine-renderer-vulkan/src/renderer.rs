mod backend;

use engine_renderer_api::{BoxedRenderer, RenderExtent, RenderWindow, Renderer, RendererFactory};
use error_stack::{Report, ResultExt};
use thiserror::Error;

use self::backend::VulkanBackend;

/// Errors returned by Vulkan renderer operations.
#[derive(Debug, Error)]
pub enum VulkanRendererError {
    /// Vulkan backend operation failed.
    #[error("vulkan backend failed")]
    Backend,

    /// Window drawable size is invalid for presentation.
    #[error("window drawable size must be non-zero: {0:?}")]
    InvalidWindowSize(RenderExtent),
}

/// Vulkan-backed renderer state.
pub struct VulkanRenderer {
    #[allow(dead_code)]
    vsync: bool,

    #[allow(dead_code)]
    backend: VulkanBackend,
}

impl std::fmt::Debug for VulkanRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanRenderer").field("vsync", &self.vsync).finish_non_exhaustive()
    }
}

/// Configures a [`VulkanRenderer`].
#[derive(Clone, Copy, Debug, Default)]
pub struct VulkanRendererBuilder {
    vsync: bool,
}

impl VulkanRendererBuilder {
    /// Builds the renderer for a native presentation window.
    pub fn build(
        self,
        window: &dyn RenderWindow,
    ) -> error_stack::Result<VulkanRenderer, VulkanRendererError> {
        let extent = window.size();

        if extent.is_empty() {
            return Err(Report::new(VulkanRendererError::InvalidWindowSize(extent)));
        }

        let backend =
            VulkanBackend::new(window, self.vsync).change_context(VulkanRendererError::Backend)?;

        VulkanRenderer::new(self.vsync, backend)
    }

    /// Enables or disables vertical synchronization.
    #[must_use]
    pub const fn with_vsync(mut self, on: bool) -> Self {
        self.vsync = on;
        self
    }
}

impl VulkanRenderer {
    /// Creates a builder for configuring a [`VulkanRenderer`].
    pub fn builder() -> VulkanRendererBuilder {
        VulkanRendererBuilder::default()
    }

    fn new(vsync: bool, backend: VulkanBackend) -> error_stack::Result<Self, VulkanRendererError> {
        Ok(Self { vsync, backend })
    }
}

impl RendererFactory for VulkanRendererBuilder {
    type Error = VulkanRendererError;

    fn create_renderer(
        &mut self,
        window: &dyn RenderWindow,
    ) -> error_stack::Result<BoxedRenderer<Self::Error>, Self::Error> {
        Ok(Box::new((*self).build(window)?))
    }
}

impl Renderer for VulkanRenderer {
    type Error = VulkanRendererError;

    fn prepare_frame(&mut self) -> error_stack::Result<(), Self::Error> {
        Ok(())
    }

    fn render(&mut self) -> error_stack::Result<(), Self::Error> {
        Ok(())
    }

    fn resize(&mut self, _extent: RenderExtent) -> error_stack::Result<(), Self::Error> {
        Ok(())
    }
}
