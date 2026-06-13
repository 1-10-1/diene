//! Shared renderer abstraction used by engine orchestration and backends.

#![forbid(unsafe_code)]

use std::{
    error::Error,
    fmt::{self, Debug, Display},
};

pub use raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle};

/// Drawable size in physical pixels.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderExtent {
    /// Drawable width in physical pixels.
    pub width: u32,

    /// Drawable height in physical pixels.
    pub height: u32,
}

impl RenderExtent {
    /// Creates a drawable extent in physical pixels.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Returns `true` when either dimension is zero.
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// Native window data needed by renderer backends.
pub trait RenderWindow: Debug + HasDisplayHandle + HasWindowHandle {
    /// Returns the current drawable window size in physical pixels.
    fn size(&self) -> RenderExtent;
}

/// Renderer backend operations driven by the application loop.
pub trait Renderer: Debug {
    /// Prepares renderer-owned state for the next frame.
    fn prepare_frame(&mut self) -> Result<(), RendererError>;

    /// Renders one frame.
    fn render(&mut self) -> Result<(), RendererError>;

    /// Resizes renderer-owned swapchain or framebuffer resources.
    fn resize(&mut self, extent: RenderExtent) -> Result<(), RendererError>;
}

/// Type-erased renderer backend error.
#[derive(Debug)]
pub struct RendererError {
    source: Box<dyn Error + Send + Sync>,
}

impl RendererError {
    /// Wraps a backend-specific renderer error.
    #[must_use]
    pub fn new(source: impl Error + Send + Sync + 'static) -> Self {
        Self { source: Box::new(source) }
    }
}

impl Display for RendererError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.source, formatter)
    }
}

impl Error for RendererError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&*self.source)
    }
}

/// Owned renderer trait object.
pub type BoxedRenderer = Box<dyn Renderer>;

/// Creates renderer instances once a native window exists.
pub trait RendererFactory: Debug {
    /// Creates a renderer for the supplied native window.
    fn create_renderer(&mut self, window: &dyn RenderWindow) -> Result<BoxedRenderer, RendererError>;
}
