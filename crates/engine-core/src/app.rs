//! Applicagion lifecycle and frame coordination.

use engine_renderer_vulkan::renderer::{self, VulkanRenderer};

/// Owns the high-level state for a running application.
#[derive(Debug)]
pub struct Application {
    name: String,
    renderer: renderer::VulkanRenderer,
}

/// Configures an [`Application`].
#[derive(Debug, Default)]
pub struct ApplicationBuilder {
    name: Option<String>,
}

impl ApplicationBuilder {
    /// Sets the human-readable application name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Builds the application.
    pub fn build(self) -> Application {
        let renderer = VulkanRenderer::builder().with_vsync(false).build();

        Application {
            name: self
                .name
                .unwrap_or_else(|| "Untitled Application".to_owned()),
            renderer,
        }
    }
}

impl Application {
    /// Creates a builder for configuring an [`Application`].
    pub fn builder() -> ApplicationBuilder {
        ApplicationBuilder::default()
    }

    /// Returns the human-readable application name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Advances the application by one frame.
    pub fn tick(&mut self) {
        self.renderer.update();
        self.renderer.render();
    }
}
