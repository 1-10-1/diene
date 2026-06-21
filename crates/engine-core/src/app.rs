//! Application lifecycle and frame coordination.

use std::error::Error as StdError;

use common::{
    logging::macros::{debug, error, info},
    timer::Stopwatch,
};
use engine_renderer_api::{BoxedRenderer, RenderExtent, RenderWindow, RendererError, RendererFactory};
use thiserror::Error;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::WindowId,
};

mod windowing;

use self::windowing::Window;
pub use self::windowing::WindowError;

/// Errors returned by application host lifecycle operations.
#[derive(Debug, Error)]
pub enum ApplicationHostError {
    /// Renderer operation failed.
    #[error("renderer failed: {0}")]
    Renderer(#[source] RendererError),

    /// Window or event loop operation failed.
    #[error("window failed: {0}")]
    Window(#[from] WindowError),

    /// Application host configuration is invalid.
    #[error("application host build failed: {0}")]
    Build(#[from] ApplicationHostBuildError),
}

/// Errors returned while building an [`ApplicationHost`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ApplicationHostBuildError {
    /// Application name cannot be empty or whitespace-only.
    #[error("application name cannot be empty")]
    EmptyName,
}

/// Drives the native window, event loop, and renderer for an application.
#[derive(Debug)]
pub struct ApplicationHost {
    name: String,
    renderer_factory: BoxedErasedRendererFactory,
    renderer: Option<BoxedErasedRenderer>,
    window: Option<Window>,
    error: Option<ApplicationHostError>,

    #[allow(dead_code)]
    stopwatch: common::timer::Stopwatch,
}

/// Configures an [`ApplicationHost`].
#[derive(Debug)]
pub struct ApplicationHostBuilder {
    name: Option<String>,
    renderer_factory: BoxedErasedRendererFactory,
}

impl ApplicationHostBuilder {
    /// Sets the human-readable application name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Builds the application host.
    pub fn build(self) -> Result<ApplicationHost, ApplicationHostError> {
        let name = self.name.unwrap_or_else(|| "Untitled Application".to_owned());

        if name.trim().is_empty() {
            return Err(ApplicationHostBuildError::EmptyName.into());
        }

        debug!("[{}] building application host", name);

        let timer = Stopwatch::new();

        Ok(ApplicationHost {
            name,
            renderer_factory: self.renderer_factory,
            renderer: None,
            window: None,
            error: None,
            stopwatch: timer,
        })
    }
}

impl ApplicationHost {
    /// Creates a builder for configuring an [`ApplicationHost`].
    pub fn builder(renderer_factory: impl RendererFactory + 'static) -> ApplicationHostBuilder {
        ApplicationHostBuilder { name: None, renderer_factory: Box::new(RendererFactoryAdapter::new(renderer_factory)) }
    }

    /// Returns the human-readable application name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Runs the application until the event loop exits.
    pub fn run(mut self) -> Result<(), ApplicationHostError> {
        info!("[{}] running application", self.name);

        let event_loop = EventLoop::new().map_err(WindowError::from)?;

        event_loop.run_app(&mut self).map_err(WindowError::from)?;

        if let Some(error) = self.error.take() {
            error!("[{}] application event loop exited with error: {:#}", self.name, error);
            return Err(error);
        }

        info!("[{}] application event loop exited gracefully", self.name);

        Ok(())
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: ApplicationHostError) {
        self.error = Some(error);
        event_loop.exit();
    }

    fn is_main_window(&self, window_id: WindowId) -> bool {
        self.window.as_ref().is_some_and(|window| window.id() == window_id)
    }

    fn render_frame(&mut self, event_loop: &ActiveEventLoop) {
        // WARN: This is only done for debug purposes. Be sure to remove it later!
        if self.stopwatch.is_stopped() {
            self.stopwatch.start();
        }

        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        if let Err(error) = renderer.prepare_frame().and_then(|()| renderer.render()) {
            self.fail(event_loop, ApplicationHostError::Renderer(error));
            return;
        }

        if let Some(window) = &self.window {
            window.request_redraw();
        }

        let elapsed = self.stopwatch.elapsed().as_secs_f32();

        if elapsed > 2.0 {
            info!("{:.0} seconds elapsed, exitting gracefully...", elapsed);
            event_loop.exit();
        }
    }

    fn resize_renderer(&mut self, event_loop: &ActiveEventLoop, size: winit::dpi::PhysicalSize<u32>) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        let extent = RenderExtent::new(size.width, size.height);

        if let Err(error) = renderer.resize(extent) {
            self.fail(event_loop, ApplicationHostError::Renderer(error));
        }
    }
}

impl ApplicationHandler for ApplicationHost {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = match Window::create(event_loop, &self.name) {
            Ok(window) => {
                info!("[{}] created application window (Id: {:?})", self.name, window.id());
                window
            }
            Err(error) => {
                self.fail(event_loop, ApplicationHostError::Window(error));
                return;
            }
        };

        let renderer = match self.renderer_factory.create_renderer(&window) {
            Ok(renderer) => {
                info!("[{}] created renderer", self.name);
                renderer
            }
            Err(error) => {
                self.fail(event_loop, ApplicationHostError::Renderer(error));
                return;
            }
        };

        window.request_redraw();

        self.renderer = Some(renderer);
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        if !self.is_main_window(window_id) {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                info!("[{}] close requested for window {:?}", self.name, window_id);
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.resize_renderer(event_loop, size);
            }
            WindowEvent::RedrawRequested => {
                self.render_frame(event_loop);
            }
            _ => {}
        }
    }
}

type BoxedErasedRenderer = Box<dyn ErasedRenderer>;
type BoxedErasedRendererFactory = Box<dyn ErasedRendererFactory>;

trait ErasedRenderer: std::fmt::Debug {
    fn prepare_frame(&mut self) -> Result<(), RendererError>;

    fn render(&mut self) -> Result<(), RendererError>;

    fn resize(&mut self, extent: RenderExtent) -> Result<(), RendererError>;
}

#[derive(Debug)]
struct RendererAdapter<E>
where
    E: StdError + Send + Sync + 'static,
{
    inner: BoxedRenderer<E>,
}

impl<E> RendererAdapter<E>
where
    E: StdError + Send + Sync + 'static,
{
    fn new(inner: BoxedRenderer<E>) -> Self {
        Self { inner }
    }
}

impl<E> ErasedRenderer for RendererAdapter<E>
where
    E: StdError + Send + Sync + 'static,
{
    fn prepare_frame(&mut self) -> Result<(), RendererError> {
        self.inner.prepare_frame().map_err(erase_renderer_error)
    }

    fn render(&mut self) -> Result<(), RendererError> {
        self.inner.render().map_err(erase_renderer_error)
    }

    fn resize(&mut self, extent: RenderExtent) -> Result<(), RendererError> {
        self.inner.resize(extent).map_err(erase_renderer_error)
    }
}

trait ErasedRendererFactory: std::fmt::Debug {
    fn create_renderer(&mut self, window: &dyn RenderWindow) -> Result<BoxedErasedRenderer, RendererError>;
}

#[derive(Debug)]
struct RendererFactoryAdapter<F>
where
    F: RendererFactory,
{
    inner: F,
}

impl<F> RendererFactoryAdapter<F>
where
    F: RendererFactory,
{
    fn new(inner: F) -> Self {
        Self { inner }
    }
}

impl<F> ErasedRendererFactory for RendererFactoryAdapter<F>
where
    F: RendererFactory + 'static,
{
    fn create_renderer(&mut self, window: &dyn RenderWindow) -> Result<BoxedErasedRenderer, RendererError> {
        let renderer = self.inner.create_renderer(window).map_err(erase_renderer_error)?;

        Ok(Box::new(RendererAdapter::new(renderer)))
    }
}

fn erase_renderer_error(error: impl StdError + Send + Sync + 'static) -> RendererError {
    Box::new(error)
}
