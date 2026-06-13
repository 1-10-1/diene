mod backend;

use common::logging::macros::*;
use engine_renderer_api::{BoxedRenderer, RenderExtent, RenderWindow, Renderer, RendererError, RendererFactory};
use thiserror::Error;

use self::backend::{VulkanBackend, VulkanBackendError};

/// Errors returned by Vulkan renderer operations.
#[derive(Debug, Error)]
pub enum VulkanRendererError {
    /// Vulkan backend operation failed.
    #[error("vulkan backend error: {0}")]
    Backend(#[from] VulkanBackendError),

    /// Window drawable size is invalid for presentation.
    #[error("window drawable size must be non-zero: {0:?}")]
    InvalidWindowSize(RenderExtent),
}

impl From<VulkanRendererError> for RendererError {
    fn from(error: VulkanRendererError) -> Self {
        Self::new(error)
    }
}

/// Vulkan-backed renderer state.
#[derive(Debug)]
pub struct VulkanRenderer {
    #[allow(dead_code)]
    vsync: bool,

    #[allow(dead_code)]
    backend: VulkanBackend,
}

/// Configures a [`VulkanRenderer`].
#[derive(Clone, Copy, Debug, Default)]
pub struct VulkanRendererBuilder {
    vsync: bool,
}

impl VulkanRendererBuilder {
    /// Builds the renderer for a native presentation window.
    pub fn build(self, window: &dyn RenderWindow) -> Result<VulkanRenderer, VulkanRendererError> {
        let extent = window.size();

        debug!("building vulkan renderer frontend");

        if extent.is_empty() {
            return Err(VulkanRendererError::InvalidWindowSize(extent));
        }

        let backend = VulkanBackend::new()?;

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

    fn new(vsync: bool, backend: VulkanBackend) -> Result<Self, VulkanRendererError> {
        debug!("initializing vulkan renderer frontend state");

        Ok(Self { vsync, backend })
    }
}

impl RendererFactory for VulkanRendererBuilder {
    fn create_renderer(&mut self, window: &dyn RenderWindow) -> Result<BoxedRenderer, RendererError> {
        Ok(Box::new((*self).build(window)?))
    }
}

impl Renderer for VulkanRenderer {
    fn prepare_frame(&mut self) -> Result<(), RendererError> {
        Ok(())
    }

    fn render(&mut self) -> Result<(), RendererError> {
        Ok(())
    }

    fn resize(&mut self, _extent: RenderExtent) -> Result<(), RendererError> {
        Ok(())
    }
}
