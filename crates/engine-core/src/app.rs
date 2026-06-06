//! Application core.

use engine_renderer_vulkan::renderer::{self, VulkanRenderer};

/// FIXME Problem. Logger needs to be shared. But that is tricky.
/// Should it be placed in the engine code? Renderer code? Sandbox? Shared?

/// Application data.
#[derive(Debug)]
pub struct Application {
    name: String,
    renderer: renderer::VulkanRenderer,
}

/// Builder for an [`Application`].
#[derive(Debug, Default)]
pub struct ApplicationBuilder {
    name: Option<String>,
}

impl ApplicationBuilder {
    /// Sets the application name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Constructs an [`Application`] from the builder state.
    pub fn build(self) -> Application {
        Application {
            name: self
                .name
                .unwrap_or_else(|| "Untitled Application".to_owned()),
            renderer: VulkanRenderer::builder().build(),
        }
    }
}

impl Application {
    /// Creates a new [`ApplicationBuilder`].
    pub fn builder() -> ApplicationBuilder {
        ApplicationBuilder::default()
    }

    /// Returns the application name.
    pub fn name(&self) -> &str {
        &self.name
    }
}
