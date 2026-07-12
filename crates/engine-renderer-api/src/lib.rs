//! Shared renderer abstraction used by engine orchestration and
//! backends.

#![forbid(unsafe_code)]

use std::{error::Error, fmt::Debug};

pub use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

/// Drawable size in physical pixels.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderExtent {
    /// Drawable width in pixels.
    pub width: u32,

    /// Drawable height in pixels.
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
    /// Error context returned by renderer operations.
    type Error: error_stack::Context;

    /// Prepares renderer-owned state for the next frame.
    fn prepare_frame(&mut self) -> error_stack::Result<(), Self::Error>;

    /// Renders one frame.
    fn render(&mut self) -> error_stack::Result<(), Self::Error>;

    /// Resizes renderer-owned swapchain or framebuffer resources.
    fn resize(&mut self, extent: RenderExtent) -> error_stack::Result<(), Self::Error>;
}

/// Owned renderer trait object with a fixed error type.
pub type BoxedRenderer<E> = Box<dyn Renderer<Error = E>>;

/// Type-erased renderer error used at dynamic renderer boundaries.
pub type RendererError = Box<dyn Error + Send + Sync + 'static>;

/// Creates renderer instances once a native window exists.
pub trait RendererFactory: Debug {
    /// Error context returned by renderer creation and operations.
    type Error: error_stack::Context;

    /// Creates a renderer for the supplied native window.
    fn create_renderer(
        &mut self,
        window: &dyn RenderWindow,
    ) -> error_stack::Result<BoxedRenderer<Self::Error>, Self::Error>;
}

/// Owned renderer factory trait object with a fixed error type.
pub type BoxedRendererFactory<E> = Box<dyn RendererFactory<Error = E>>;
