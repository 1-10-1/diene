//! Engine runtime with renderer backend selection policy.

#![forbid(unsafe_code)]

use common::logging::macros::{debug, info};
use engine_core::app::{
    ApplicationHost, ApplicationHostBuildError, ApplicationHostError, WindowError,
};
use engine_renderer_api::{
    BoxedRenderer, RenderExtent, RenderScene, RenderWindow, Renderer, RendererError,
    RendererFactory,
};
use engine_renderer_vulkan::VulkanRendererBuilder;
use error_stack::ResultExt;
use thiserror::Error;

/// Errors returned by the public application runtime.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ApplicationError {
    /// Renderer backend operation failed.
    #[error("renderer failed: {0}")]
    Renderer(#[source] RendererError),

    /// Window or event loop operation failed.
    #[error("window failed: {0}")]
    Window(#[from] WindowError),

    /// Application configuration is invalid.
    #[error("application build failed: {0}")]
    Build(#[from] ApplicationHostBuildError),
}

impl From<ApplicationHostError> for ApplicationError {
    fn from(error: ApplicationHostError) -> Self {
        match error {
            ApplicationHostError::Renderer(error) => Self::Renderer(error),
            ApplicationHostError::Window(error) => Self::Window(error),
            ApplicationHostError::Build(error) => Self::Build(error),
        }
    }
}

/// Errors returned by compiled renderer backends.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RendererBackendError {
    /// Vulkan backend operation failed.
    #[error("vulkan renderer failed")]
    Vulkan,
}

/// Renderer backend selection mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum RendererBackend {
    /// Chooses the best supported backend compiled into this binary.
    #[default]
    Auto,

    /// Forces the Vulkan backend.
    Vulkan,
}

const DEFAULT_RENDERER_BACKEND: RendererBackend = RendererBackend::Vulkan;

/// Public engine application that owns renderer backend selection
/// policy.
#[derive(Debug)]
pub struct Application {
    host: ApplicationHost,
}

/// Configures an [`Application`].
#[derive(Debug, Default)]
pub struct ApplicationBuilder {
    name: Option<String>,
    renderer_backend: RendererBackend,
    scene: RenderScene,
    vsync: bool,
}

impl ApplicationBuilder {
    /// Sets the human-readable application name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Selects the renderer backend policy.
    #[must_use]
    pub fn with_renderer_backend(mut self, backend: RendererBackend) -> Self {
        self.renderer_backend = backend;
        self
    }

    /// Enables or disables vertical synchronization.
    #[must_use]
    pub fn with_vsync(mut self, on: bool) -> Self {
        self.vsync = on;
        self
    }

    /// Sets the initial renderer scene.
    #[must_use]
    pub fn with_scene(mut self, scene: RenderScene) -> Self {
        self.scene = scene;
        self
    }

    /// Builds the application.
    pub fn build(self) -> Result<Application, ApplicationError> {
        debug!("building runtime application with backend {:?}", self.renderer_backend);

        let selector = RendererBackendSelector {
            renderer_backend: self.renderer_backend,
            scene: self.scene,
            vsync: self.vsync,
        };

        let mut builder = ApplicationHost::builder(selector);

        if let Some(name) = self.name {
            builder = builder.with_name(name);
        }

        Ok(Application { host: builder.build()? })
    }
}

impl Application {
    /// Creates a builder for configuring an [`Application`].
    pub fn builder() -> ApplicationBuilder {
        ApplicationBuilder::default()
    }

    /// Returns the human-readable application name.
    pub fn name(&self) -> &str {
        self.host.name()
    }

    /// Runs the application until the event loop exits.
    pub fn run(self) -> Result<(), ApplicationError> {
        self.host.run().map_err(Into::into)
    }
}

#[derive(Clone, Debug)]
struct RendererBackendSelector {
    renderer_backend: RendererBackend,
    scene: RenderScene,
    vsync: bool,
}

impl RendererFactory for RendererBackendSelector {
    type Error = RendererBackendError;

    fn create_renderer(
        &mut self,
        window: &dyn RenderWindow,
    ) -> error_stack::Result<BoxedRenderer<Self::Error>, Self::Error> {
        let auto = self.renderer_backend == RendererBackend::Auto;

        if auto {
            self.renderer_backend = DEFAULT_RENDERER_BACKEND;
        }

        info!(
            "selected renderer backend {:?}{}",
            self.renderer_backend,
            if auto {
                " (auto)"
            } else {
                ""
            }
        );

        match self.renderer_backend {
            RendererBackend::Vulkan => create_vulkan_renderer(window, self.vsync, &self.scene),
            RendererBackend::Auto => unreachable!(),
        }
    }
}

#[derive(Debug)]
struct RendererErrorAdapter<R> {
    inner: R,
}

impl<R> RendererErrorAdapter<R> {
    fn new(inner: R) -> Self {
        Self { inner }
    }
}

impl<R> Renderer for RendererErrorAdapter<R>
where
    R: Renderer,
{
    type Error = RendererBackendError;

    fn prepare_frame(&mut self) -> error_stack::Result<(), Self::Error> {
        self.inner.prepare_frame().change_context(RendererBackendError::Vulkan)
    }

    fn render(&mut self) -> error_stack::Result<(), Self::Error> {
        self.inner.render().change_context(RendererBackendError::Vulkan)
    }

    fn resize(&mut self, extent: RenderExtent) -> error_stack::Result<(), Self::Error> {
        self.inner.resize(extent).change_context(RendererBackendError::Vulkan)
    }
}

fn create_vulkan_renderer(
    window: &dyn RenderWindow,
    vsync: bool,
    scene: &RenderScene,
) -> error_stack::Result<BoxedRenderer<RendererBackendError>, RendererBackendError> {
    let renderer = VulkanRendererBuilder::default()
        .with_vsync(vsync)
        .with_scene(scene.clone())
        .build(window)
        .change_context(RendererBackendError::Vulkan)?;

    Ok(Box::new(RendererErrorAdapter::new(renderer)))
}
