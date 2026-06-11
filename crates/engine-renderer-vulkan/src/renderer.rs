/// Vulkan-backed renderer state.
#[derive(Debug, Default)]
pub struct VulkanRenderer {
    #[allow(dead_code)]
    vsync: bool,
}

/// Configures a [`VulkanRenderer`].
#[derive(Debug, Default)]
pub struct VulkanRendererBuilder {
    vsync: bool,
}

impl VulkanRendererBuilder {
    /// Builds the renderer.
    pub const fn build(self) -> VulkanRenderer {
        VulkanRenderer { vsync: self.vsync }
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

    /// Renders one frame.
    pub fn render(&mut self) {}

    /// Updates per-frame renderer state.
    pub fn update(&mut self) {}
}
