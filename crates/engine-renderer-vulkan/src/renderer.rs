/// Vulkan renderer data
#[derive(Debug, Default)]
pub struct VulkanRenderer {
    vsync: bool,
}

/// Builder for a [`VulkanRenderer`]
#[derive(Debug, Default)]
pub struct VulkanRendererBuilder {
    vsync: bool,
}

impl VulkanRendererBuilder {
    /// Constructs a [`VulkanRenderer`] from the builder state.
    pub const fn build(self) -> VulkanRenderer {
        VulkanRenderer { vsync: self.vsync }
    }

    /// Sets the vertical sync
    #[must_use]
    pub const fn with_vsync(mut self, on: bool) -> Self {
        self.vsync = on;
        self
    }
}

impl VulkanRenderer {
    /// Creates a new [`VulkanRendererBuilder`]
    pub fn builder() -> VulkanRendererBuilder {
        VulkanRendererBuilder::default()
    }

    /// Renders into the attached window.
    pub fn render(&mut self) {
        println!("Hello!");
    }

    /// Updates the renderer state.
    /// Should be called for each game tick, and before [`VulkanRenderer::render()`]
    /// is called.
    pub fn update(&mut self) {
        println!("Nice place!");
    }
}
