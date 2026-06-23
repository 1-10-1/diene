//! Thin wrapper around native window creation and access.

use engine_renderer_api::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RenderExtent, RenderWindow,
    WindowHandle,
};
use thiserror::Error;
use winit::{
    dpi::PhysicalSize,
    event_loop::ActiveEventLoop,
    window::{Window as WinitWindow, WindowId},
};

/// Errors returned by native window operations.
#[derive(Debug, Error)]
pub enum WindowError {
    /// Event loop operation failed.
    #[error("event loop failed: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),

    /// Operating system call failed.
    #[error("operating system call failed: {0}")]
    Os(#[from] winit::error::OsError),
}

/// Owns the native window used for presenting rendered frames.
#[derive(Debug)]
pub(super) struct Window {
    inner: WinitWindow,
}

impl Window {
    /// Creates a window attached to the active event loop.
    pub(super) fn create(event_loop: &ActiveEventLoop, title: &str) -> Result<Self, WindowError> {
        let attributes = WinitWindow::default_attributes().with_title(title);
        let inner = event_loop.create_window(attributes)?;

        Ok(Self { inner })
    }

    /// Returns the platform window identifier.
    pub(super) fn id(&self) -> WindowId {
        self.inner.id()
    }

    /// Returns the current drawable size in physical pixels.
    fn physical_size(&self) -> PhysicalSize<u32> {
        self.inner.inner_size()
    }

    /// Requests that the event loop schedule a redraw for this window.
    pub(super) fn request_redraw(&self) {
        self.inner.request_redraw();
    }
}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.inner.window_handle()
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.inner.display_handle()
    }
}

impl RenderWindow for Window {
    fn size(&self) -> RenderExtent {
        let size = self.physical_size();
        RenderExtent::new(size.width, size.height)
    }
}
