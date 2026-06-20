//! Engine runtime with renderer backend selection policy.

#![forbid(unsafe_code)]

use common::logging::macros::{debug, info};
use engine_core::app::{ApplicationHost, ApplicationHostBuildError, ApplicationHostError, WindowError};
use engine_renderer_api::{BoxedRenderer, RenderWindow, RendererFactory};
use engine_renderer_vulkan::renderer::{VulkanRendererBuilder, VulkanRendererError};
use thiserror::Error;

/// Errors returned by the public application runtime.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ApplicationError {
    /// Renderer backend operation failed.
    #[error("renderer failed: {0}")]
    Renderer(#[source] VulkanRendererError),

    /// Window or event loop operation failed.
    #[error("window failed: {0}")]
    Window(#[from] WindowError),

    /// Application configuration is invalid.
    #[error("application build failed: {0}")]
    Build(#[from] ApplicationHostBuildError),
}

impl From<ApplicationHostError<VulkanRendererError>> for ApplicationError {
    fn from(error: ApplicationHostError<VulkanRendererError>) -> Self {
        match error {
            ApplicationHostError::Renderer(error) => Self::Renderer(error),
            ApplicationHostError::Window(error) => Self::Window(error),
            ApplicationHostError::Build(error) => Self::Build(error),
        }
    }
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

/// Public engine application that owns renderer backend selection policy.
#[derive(Debug)]
pub struct Application {
    host: RuntimeApplicationHost,
}

type RuntimeApplicationHost = ApplicationHost<RendererBackendSelector>;

/// Configures an [`Application`].
#[derive(Debug, Default)]
pub struct ApplicationBuilder {
    name: Option<String>,
    renderer_backend: RendererBackend,
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

    /// Builds the application.
    pub fn build(self) -> Result<Application, ApplicationError> {
        debug!("building runtime application with backend {:?}", self.renderer_backend);

        let selector = RendererBackendSelector { renderer_backend: self.renderer_backend, vsync: self.vsync };

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

#[derive(Clone, Copy, Debug)]
struct RendererBackendSelector {
    renderer_backend: RendererBackend,
    vsync: bool,
}

impl RendererFactory for RendererBackendSelector {
    // FIXME: What the hell?
    type Error = VulkanRendererError;

    fn create_renderer(&mut self, window: &dyn RenderWindow) -> Result<BoxedRenderer<Self::Error>, Self::Error> {
        match self.renderer_backend {
            RendererBackend::Auto | RendererBackend::Vulkan => {
                info!(
                    "selected renderer backend vulkan with {} policy",
                    if matches!(self.renderer_backend, RendererBackend::Auto) {
                        "auto"
                    } else {
                        "forced"
                    }
                );
                create_vulkan_renderer(window, self.vsync)
            }
        }
    }
}

fn create_vulkan_renderer(window: &dyn RenderWindow, vsync: bool) -> Result<BoxedRenderer<VulkanRendererError>, VulkanRendererError> {
    debug!("creating vulkan renderer");

    let renderer = VulkanRendererBuilder::default().with_vsync(vsync).build(window)?;

    Ok(Box::new(renderer))
}
