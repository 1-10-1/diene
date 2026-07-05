use ash::vk;
use thiserror::Error;

use crate::renderer::backend::call_error::VulkanCallError;

/// Errors returned by Vulkan backend operations.
#[derive(Debug, Error)]
pub(super) enum VulkanPipelineError {
    /// Vulkan API call returned an error value.
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),
}

#[allow(clippy::struct_excessive_bools)]
struct GraphicsPipeline {
    // Depth Testing
    depth_test_enable: bool,
    depth_write_enable: bool,
    depth_bounds_test: bool,
    stencil_enable: bool,
    depth_compare: vk::CompareOp,

    // IA
    primitive_restart: bool,
    primitive_topology: vk::PrimitiveTopology,

    // Vertex State
    vertex_binding_descs: Vec<vk::VertexInputBindingDescription>,
    vertex_attribute_descs: Vec<vk::VertexInputAttributeDescription>,

    // Rasterizer
    depth_clamp_enabled: bool,
    rasterizer_discard: bool,
    depth_bias_enabled: bool,
    line_width: f32,
    polygon_mode: vk::PolygonMode,
    cull_mode: vk::CullModeFlags,
    front_face: vk::FrontFace,

    viewport_count: u32,
    scissor_count: u32,
    depth_bias_constant_factor: f32,
    depth_bias_clamp: f32,
    depth_bias_slope_factor: f32,

    // multisampling
    sample_shading_enable: bool,
    alpha_to_coverage_enable: bool,
    alpha_to_one_enable: bool,

    rasterization_samples: vk::SampleCountFlags,
    min_sample_shading: f32,
    sample_mask: Option<vk::SampleMask>,

    // blending
    blending_enable: bool,
    blending_color_write_mask: vk::ColorComponentFlags,

    // Dynamic rendering
    color_attachment_format: Option<vk::Format>,
    depth_attachment_format: Option<vk::Format>,
}

impl Default for GraphicsPipeline {
    fn default() -> Self {
        Self {
            // Depth testing
            depth_test_enable: false,
            depth_write_enable: true,
            depth_bounds_test: false,
            stencil_enable: false,
            depth_compare: vk::CompareOp::LESS,

            // IA
            primitive_restart: false,
            primitive_topology: vk::PrimitiveTopology::TRIANGLE_LIST,

            // Vertex State
            vertex_binding_descs: Vec::default(),
            vertex_attribute_descs: Vec::default(),

            // Rasterizer
            depth_clamp_enabled: false,
            rasterizer_discard: false,
            depth_bias_enabled: false,
            line_width: 1.0,
            polygon_mode: vk::PolygonMode::FILL,
            cull_mode: vk::CullModeFlags::NONE,
            front_face: vk::FrontFace::CLOCKWISE,

            viewport_count: 1,
            scissor_count: 1,

            depth_bias_constant_factor: 0.0,
            depth_bias_clamp: 0.0,
            depth_bias_slope_factor: 0.0,

            // Multisampling
            sample_shading_enable: false,
            alpha_to_coverage_enable: false,
            alpha_to_one_enable: false,
            rasterization_samples: vk::SampleCountFlags::TYPE_1,
            min_sample_shading: 0.3,
            sample_mask: None,

            // Blending
            blending_enable: true,
            blending_color_write_mask: vk::ColorComponentFlags::RGBA,

            // Dynamic Rendering
            color_attachment_format: None,
            depth_attachment_format: None,
        }
    }
}
