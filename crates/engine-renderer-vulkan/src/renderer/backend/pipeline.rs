#![allow(dead_code)]

use std::{
    ffi::CString,
    fs, io, mem,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use ash::vk::{self, Handle};
use common::logging::macros::*;
use thiserror::Error;

use crate::renderer::backend::{
    call_error::VulkanCallError,
    device::{VulkanDevice, VulkanLogicalDevice},
    shader::VulkanShader,
};

const PIPELINE_CACHE_DIR: &str = "cache";

/// Errors returned by Vulkan pipeline operations.
#[derive(Debug, Error)]
pub(super) enum VulkanPipelineError {
    /// Vulkan API call returned an error value.
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),

    /// Required graphics pipeline builder field was not provided.
    #[error("graphics pipeline builder is missing {0}")]
    IncompleteGraphicsPipeline(&'static str),

    /// Pipeline creation succeeded without returning a valid pipeline handle.
    #[error("graphics pipeline creation did not return a pipeline handle")]
    NoPipelineReturned,

    /// Pipeline name could not be used as a cache file stem.
    #[error("invalid pipeline name `{name}`")]
    InvalidPipelineName {
        /// Name that failed validation.
        name: String,
    },

    /// Pipeline cache directory could not be created.
    #[error("failed to create pipeline cache directory `{path}`: {source}")]
    CacheDirectoryCreation {
        /// Cache directory path.
        path: PathBuf,

        /// Filesystem error.
        #[source]
        source: io::Error,
    },

    /// Pipeline cache data could not be written.
    #[error("failed to write pipeline cache `{path}`: {source}")]
    CacheWrite {
        /// Cache file path.
        path: PathBuf,

        /// Filesystem error.
        #[source]
        source: io::Error,
    },
}

/// Builds a Vulkan pipeline layout.
#[derive(Clone, Debug, Default)]
pub(super) struct VulkanPipelineLayoutBuilder {
    push_constants: Option<vk::PushConstantRange>,
    descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
}

impl VulkanPipelineLayoutBuilder {
    /// Sets a single push-constant range starting at offset zero.
    pub(super) fn with_push_constants(
        mut self,
        size: u32,
        stage_flags: vk::ShaderStageFlags,
    ) -> Self {
        self.push_constants =
            Some(vk::PushConstantRange::default().stage_flags(stage_flags).offset(0).size(size));
        self
    }

    /// Sets descriptor-set layouts used by pipelines created from this layout.
    pub(super) fn with_descriptor_set_layouts<I>(mut self, layouts: I) -> Self
    where
        I: IntoIterator<Item = vk::DescriptorSetLayout>,
    {
        self.descriptor_set_layouts = layouts.into_iter().collect();
        self
    }

    /// Builds the configured Vulkan pipeline layout.
    pub(super) fn build(
        self,
        device: Arc<VulkanLogicalDevice>,
    ) -> core::result::Result<VulkanPipelineLayout, VulkanPipelineError> {
        let push_constant_ranges = self.push_constants.into_iter().collect::<Vec<_>>();
        let create_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&self.descriptor_set_layouts)
            .push_constant_ranges(&push_constant_ranges);

        // SAFETY: `create_info` only references local slices that live through the call.
        let handle = vk_try!("create pipeline layout", unsafe {
            device.handle().create_pipeline_layout(&create_info, None)
        });

        let layout = VulkanPipelineLayout { device, handle };

        #[cfg(debug_assertions)]
        vk_try!("name pipeline layout", layout.device.set_name(c"pipeline layout", layout.handle),);

        Ok(layout)
    }
}

/// Owns a Vulkan pipeline layout.
pub(super) struct VulkanPipelineLayout {
    device: Arc<VulkanLogicalDevice>,
    handle: vk::PipelineLayout,
}

impl std::fmt::Debug for VulkanPipelineLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanPipelineLayout").field("handle", &self.handle).finish_non_exhaustive()
    }
}

impl Drop for VulkanPipelineLayout {
    fn drop(&mut self) {
        // SAFETY: `self.handle` was created through `self.device` and is destroyed exactly once.
        unsafe {
            self.device.handle().destroy_pipeline_layout(self.handle, None);
        }
    }
}

impl VulkanPipelineLayout {
    /// Creates a builder for a Vulkan pipeline layout.
    pub(super) fn builder() -> VulkanPipelineLayoutBuilder {
        VulkanPipelineLayoutBuilder::default()
    }

    /// Returns the underlying Vulkan pipeline layout handle.
    pub(super) fn get(&self) -> vk::PipelineLayout {
        self.handle
    }
}

/// Builds a Vulkan graphics pipeline.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug)]
pub(super) struct VulkanGraphicsPipelineBuilder<'a> {
    shaders: Vec<&'a VulkanShader>,

    depth_test_enable: bool,
    depth_write_enable: bool,
    depth_bounds_test: bool,
    stencil_enable: bool,
    depth_compare: vk::CompareOp,

    primitive_restart: bool,
    primitive_topology: vk::PrimitiveTopology,

    vertex_binding_descs: Vec<vk::VertexInputBindingDescription>,
    vertex_attribute_descs: Vec<vk::VertexInputAttributeDescription>,

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

    sample_shading_enable: bool,
    alpha_to_coverage_enable: bool,
    alpha_to_one_enable: bool,
    rasterization_samples: vk::SampleCountFlags,
    min_sample_shading: f32,
    sample_mask: Option<vk::SampleMask>,

    blending_enable: bool,
    src_color_blend_factor: vk::BlendFactor,
    dst_color_blend_factor: vk::BlendFactor,
    color_blend_op: vk::BlendOp,
    src_alpha_blend_factor: vk::BlendFactor,
    dst_alpha_blend_factor: vk::BlendFactor,
    alpha_blend_op: vk::BlendOp,
    blending_color_write_mask: vk::ColorComponentFlags,

    color_attachment_format: Option<vk::Format>,
    depth_attachment_format: Option<vk::Format>,
}

impl Default for VulkanGraphicsPipelineBuilder<'_> {
    fn default() -> Self {
        Self {
            shaders: Vec::default(),

            depth_test_enable: false,
            depth_write_enable: true,
            depth_bounds_test: false,
            stencil_enable: false,
            depth_compare: vk::CompareOp::LESS,

            primitive_restart: false,
            primitive_topology: vk::PrimitiveTopology::TRIANGLE_LIST,

            vertex_binding_descs: Vec::default(),
            vertex_attribute_descs: Vec::default(),

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

            sample_shading_enable: false,
            alpha_to_coverage_enable: false,
            alpha_to_one_enable: false,
            rasterization_samples: vk::SampleCountFlags::TYPE_1,
            min_sample_shading: 0.3,
            sample_mask: None,

            blending_enable: true,
            src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
            dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::ONE,
            dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
            blending_color_write_mask: vk::ColorComponentFlags::RGBA,

            color_attachment_format: None,
            depth_attachment_format: None,
        }
    }
}

impl<'a> VulkanGraphicsPipelineBuilder<'a> {
    /// Uses the provided shader modules as graphics pipeline stages.
    pub(super) fn with_shaders<I>(mut self, shaders: I) -> Self
    where
        I: IntoIterator<Item = &'a VulkanShader>,
    {
        self.shaders = shaders.into_iter().collect();
        self
    }

    /// Appends one shader module to the graphics pipeline stages.
    pub(super) fn with_shader(mut self, shader: &'a VulkanShader) -> Self {
        self.shaders.push(shader);
        self
    }

    /// Enables or disables color blending.
    pub(super) fn with_blending_enabled(mut self, enabled: bool) -> Self {
        self.blending_enable = enabled;
        self
    }

    /// Uses standard source-alpha blending.
    pub(super) fn with_alpha_blend(mut self) -> Self {
        self.blending_enable = true;
        self.src_color_blend_factor = vk::BlendFactor::SRC_ALPHA;
        self.dst_color_blend_factor = vk::BlendFactor::ONE_MINUS_SRC_ALPHA;
        self.color_blend_op = vk::BlendOp::ADD;
        self.src_alpha_blend_factor = vk::BlendFactor::ONE;
        self.dst_alpha_blend_factor = vk::BlendFactor::ZERO;
        self.alpha_blend_op = vk::BlendOp::ADD;
        self
    }

    /// Uses additive color blending.
    pub(super) fn with_additive_blend(mut self) -> Self {
        self.blending_enable = true;
        self.src_color_blend_factor = vk::BlendFactor::SRC_ALPHA;
        self.dst_color_blend_factor = vk::BlendFactor::ONE;
        self.color_blend_op = vk::BlendOp::ADD;
        self.src_alpha_blend_factor = vk::BlendFactor::ONE;
        self.dst_alpha_blend_factor = vk::BlendFactor::ONE;
        self.alpha_blend_op = vk::BlendOp::ADD;
        self
    }

    /// Sets the color attachment write mask used by blending.
    pub(super) fn with_blending_write_mask(mut self, mask: vk::ColorComponentFlags) -> Self {
        self.blending_color_write_mask = mask;
        self
    }

    /// Sets depth test behavior.
    pub(super) fn with_depth_test(mut self, enabled: bool, compare: vk::CompareOp) -> Self {
        self.depth_test_enable = enabled;
        self.depth_compare = compare;
        self
    }

    /// Enables or disables depth writes.
    pub(super) fn with_depth_write(mut self, enabled: bool) -> Self {
        self.depth_write_enable = enabled;
        self
    }

    /// Enables or disables depth bounds testing.
    pub(super) fn with_depth_bounds_test(mut self, enabled: bool) -> Self {
        self.depth_bounds_test = enabled;
        self
    }

    /// Enables or disables stencil testing.
    pub(super) fn with_stencil_test(mut self, enabled: bool) -> Self {
        self.stencil_enable = enabled;
        self
    }

    /// Sets input assembly behavior.
    pub(super) fn with_primitive_settings(
        mut self,
        primitive_restart: bool,
        primitive_topology: vk::PrimitiveTopology,
    ) -> Self {
        self.primitive_restart = primitive_restart;
        self.primitive_topology = primitive_topology;
        self
    }

    /// Enables or disables rasterizer discard.
    pub(super) fn with_rasterizer_discard(mut self, enabled: bool) -> Self {
        self.rasterizer_discard = enabled;
        self
    }

    /// Enables or disables depth clamping.
    pub(super) fn with_depth_clamp(mut self, enabled: bool) -> Self {
        self.depth_clamp_enabled = enabled;
        self
    }

    /// Sets rasterized line width.
    pub(super) fn with_line_width(mut self, width: f32) -> Self {
        self.line_width = width;
        self
    }

    /// Sets polygon rasterization mode.
    pub(super) fn with_polygon_mode(mut self, mode: vk::PolygonMode) -> Self {
        self.polygon_mode = mode;
        self
    }

    /// Sets face culling behavior.
    pub(super) fn with_culling(
        mut self,
        cull_mode: vk::CullModeFlags,
        front_face: vk::FrontFace,
    ) -> Self {
        self.cull_mode = cull_mode;
        self.front_face = front_face;
        self
    }

    /// Sets dynamic viewport and scissor counts.
    pub(super) fn with_viewport_scissor_count(
        mut self,
        viewport_count: u32,
        scissor_count: u32,
    ) -> Self {
        self.viewport_count = viewport_count;
        self.scissor_count = scissor_count;
        self
    }

    /// Sets sample shading behavior.
    pub(super) fn with_sample_shading(mut self, enabled: bool, min_sample_shading: f32) -> Self {
        self.sample_shading_enable = enabled;
        self.min_sample_shading = min_sample_shading;
        self
    }

    /// Enables or disables alpha-to-one multisampling behavior.
    pub(super) fn with_alpha_to_one(mut self, enabled: bool) -> Self {
        self.alpha_to_one_enable = enabled;
        self
    }

    /// Enables or disables alpha-to-coverage multisampling behavior.
    pub(super) fn with_alpha_to_coverage(mut self, enabled: bool) -> Self {
        self.alpha_to_coverage_enable = enabled;
        self
    }

    /// Sets the multisample mask.
    pub(super) fn with_sample_mask(mut self, mask: vk::SampleMask) -> Self {
        self.sample_mask = Some(mask);
        self
    }

    /// Clears the multisample mask override.
    pub(super) fn without_sample_mask(mut self) -> Self {
        self.sample_mask = None;
        self
    }

    /// Sets the rasterization sample count.
    pub(super) fn with_sample_count(mut self, count: vk::SampleCountFlags) -> Self {
        self.rasterization_samples = count;
        self
    }

    /// Sets depth bias behavior.
    pub(super) fn with_depth_bias(
        mut self,
        enabled: bool,
        constant_factor: f32,
        slope_factor: f32,
        clamp: f32,
    ) -> Self {
        self.depth_bias_enabled = enabled;
        self.depth_bias_constant_factor = constant_factor;
        self.depth_bias_slope_factor = slope_factor;
        self.depth_bias_clamp = clamp;
        self
    }

    /// Sets the dynamic-rendering color attachment format.
    pub(super) fn with_color_attachment_format(mut self, format: vk::Format) -> Self {
        self.color_attachment_format = Some(format);
        self
    }

    /// Sets the dynamic-rendering depth attachment format.
    pub(super) fn with_depth_attachment_format(mut self, format: vk::Format) -> Self {
        self.depth_attachment_format = Some(format);
        self
    }

    /// Sets vertex input binding and attribute descriptions.
    pub(super) fn with_vertex_input<I, J>(mut self, bindings: I, attributes: J) -> Self
    where
        I: IntoIterator<Item = vk::VertexInputBindingDescription>,
        J: IntoIterator<Item = vk::VertexInputAttributeDescription>,
    {
        self.vertex_binding_descs = bindings.into_iter().collect();
        self.vertex_attribute_descs = attributes.into_iter().collect();
        self
    }

    /// Builds a Vulkan graphics pipeline using dynamic rendering.
    pub(super) fn build(
        self,
        device: &VulkanDevice,
        name: impl Into<String>,
        layout: &VulkanPipelineLayout,
    ) -> core::result::Result<VulkanGraphicsPipeline, VulkanPipelineError> {
        let name = name.into();

        if !is_valid_pipeline_name(&name) {
            return Err(VulkanPipelineError::InvalidPipelineName { name });
        }

        let color_attachment_format = self
            .color_attachment_format
            .ok_or(VulkanPipelineError::IncompleteGraphicsPipeline("color attachment format"))?;

        let depth_attachment_format = self
            .depth_attachment_format
            .ok_or(VulkanPipelineError::IncompleteGraphicsPipeline("depth attachment format"))?;

        let shader_stages =
            self.shaders.iter().flat_map(|shader| shader.stage_infos()).collect::<Vec<_>>();

        if shader_stages.is_empty() {
            return Err(VulkanPipelineError::IncompleteGraphicsPipeline("shader stages"));
        }

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(self.blending_enable)
            .src_color_blend_factor(self.src_color_blend_factor)
            .dst_color_blend_factor(self.dst_color_blend_factor)
            .color_blend_op(self.color_blend_op)
            .src_alpha_blend_factor(self.src_alpha_blend_factor)
            .dst_alpha_blend_factor(self.dst_alpha_blend_factor)
            .alpha_blend_op(self.alpha_blend_op)
            .color_write_mask(self.blending_color_write_mask);

        let color_blend_attachments = [color_blend_attachment];

        let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(false)
            .logic_op(vk::LogicOp::COPY)
            .attachments(&color_blend_attachments);

        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&self.vertex_binding_descs)
            .vertex_attribute_descriptions(&self.vertex_attribute_descs);

        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(self.depth_test_enable)
            .depth_write_enable(self.depth_write_enable)
            .depth_compare_op(self.depth_compare)
            .depth_bounds_test_enable(self.depth_bounds_test)
            .stencil_test_enable(self.stencil_enable);

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(self.primitive_topology)
            .primitive_restart_enable(self.primitive_restart);

        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(self.viewport_count)
            .scissor_count(self.scissor_count);

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(self.depth_clamp_enabled)
            .rasterizer_discard_enable(self.rasterizer_discard)
            .polygon_mode(self.polygon_mode)
            .cull_mode(self.cull_mode)
            .front_face(self.front_face)
            .depth_bias_enable(self.depth_bias_enabled)
            .depth_bias_constant_factor(self.depth_bias_constant_factor)
            .depth_bias_clamp(self.depth_bias_clamp)
            .depth_bias_slope_factor(self.depth_bias_slope_factor)
            .line_width(self.line_width);

        let sample_mask = self.sample_mask.map(|mask| [mask]);

        let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(self.rasterization_samples)
            .sample_shading_enable(self.sample_shading_enable)
            .min_sample_shading(self.min_sample_shading)
            .alpha_to_coverage_enable(self.alpha_to_coverage_enable)
            .alpha_to_one_enable(self.alpha_to_one_enable);

        let multisampling = sample_mask
            .as_ref()
            .map_or(multisampling, |sample_mask| multisampling.sample_mask(sample_mask));

        let color_attachment_formats = [color_attachment_format];

        let mut rendering = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&color_attachment_formats)
            .depth_attachment_format(depth_attachment_format);

        let create_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            .depth_stencil_state(&depth_stencil)
            .color_blend_state(&color_blending)
            .dynamic_state(&dynamic_state)
            .layout(layout.get())
            .base_pipeline_index(-1)
            .base_pipeline_handle(vk::Pipeline::null())
            .push_next(&mut rendering);

        let cache_path = pipeline_cache_path(&name);
        let cache_data = load_compatible_pipeline_cache(&cache_path, device.properties());

        let new_cache = cache_data.is_none();

        let cache_create_info =
            cache_data.as_ref().map_or_else(vk::PipelineCacheCreateInfo::default, |cache_data| {
                vk::PipelineCacheCreateInfo::default().initial_data(cache_data)
            });

        let logical = device.logical().clone();

        let pipeline_cache = create_pipeline_cache(logical.clone(), &name, &cache_create_info)?;

        let cache_handle =
            pipeline_cache.as_ref().map_or(vk::PipelineCache::null(), PipelineCache::handle);

        let start = Instant::now();

        // SAFETY: `create_info` references only local state that lives through the call. Shader
        // modules and the pipeline layout are live for the duration of creation.
        let handle = match unsafe {
            logical.handle().create_graphics_pipelines(cache_handle, &[create_info], None)
        } {
            Ok(pipelines) => {
                pipelines.into_iter().next().ok_or(VulkanPipelineError::NoPipelineReturned)?
            }
            Err((pipelines, result)) => {
                for pipeline in pipelines.into_iter().filter(|pipeline| !pipeline.is_null()) {
                    // SAFETY: Partial pipeline handles returned by this failed creation call belong
                    // to `logical` and have no other owner.
                    unsafe {
                        logical.handle().destroy_pipeline(pipeline, None);
                    }
                }

                return Err(VulkanCallError::new("create graphics pipeline", result).into());
            }
        };

        let pipeline = VulkanGraphicsPipeline { device: logical, handle, name };

        #[cfg(debug_assertions)]
        if let Ok(debug_name) = CString::new(format!("graphics pipeline: {}", pipeline.name))
            && let Err(result) = pipeline.device.set_name(debug_name.as_c_str(), pipeline.handle)
        {
            return Err(VulkanCallError::new("name graphics pipeline", result).into());
        }

        let cache_status = if new_cache {
            "without"
        } else {
            "with"
        };

        debug!(
            "took {:.2}ms to create graphics pipeline {} {} a cache",
            start.elapsed().as_secs_f64() * 1000.0,
            pipeline.name,
            cache_status,
        );

        if new_cache && let Some(pipeline_cache) = &pipeline_cache {
            // SAFETY: `pipeline_cache` is a live cache created through `pipeline.device`.
            let cache_data = vk_try!("get pipeline cache data", unsafe {
                pipeline.device.handle().get_pipeline_cache_data(pipeline_cache.handle())
            });
            write_pipeline_cache(&cache_path, &cache_data)?;
        }

        Ok(pipeline)
    }
}

/// Owns a Vulkan graphics pipeline.
pub(super) struct VulkanGraphicsPipeline {
    device: Arc<VulkanLogicalDevice>,
    handle: vk::Pipeline,
    name: String,
}

impl std::fmt::Debug for VulkanGraphicsPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanGraphicsPipeline")
            .field("handle", &self.handle)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl Drop for VulkanGraphicsPipeline {
    fn drop(&mut self) {
        // SAFETY: `self.handle` was created through `self.device` and is destroyed exactly once.
        unsafe {
            self.device.handle().destroy_pipeline(self.handle, None);
        }
    }
}

impl VulkanGraphicsPipeline {
    /// Creates a builder for a Vulkan graphics pipeline.
    pub(super) fn builder<'a>() -> VulkanGraphicsPipelineBuilder<'a> {
        VulkanGraphicsPipelineBuilder::default()
    }

    /// Returns the underlying Vulkan pipeline handle.
    pub(super) fn get(&self) -> vk::Pipeline {
        self.handle
    }
}

struct PipelineCache {
    device: Arc<VulkanLogicalDevice>,
    handle: vk::PipelineCache,
}

impl PipelineCache {
    fn handle(&self) -> vk::PipelineCache {
        self.handle
    }
}

impl Drop for PipelineCache {
    fn drop(&mut self) {
        // SAFETY: `self.handle` was created through `self.device` and is destroyed exactly once.
        unsafe {
            self.device.handle().destroy_pipeline_cache(self.handle, None);
        }
    }
}

fn create_pipeline_cache(
    device: Arc<VulkanLogicalDevice>,
    name: &str,
    create_info: &vk::PipelineCacheCreateInfo<'_>,
) -> core::result::Result<Option<PipelineCache>, VulkanPipelineError> {
    // SAFETY: `create_info` references compatible cache bytes that live through the call.
    match unsafe { device.handle().create_pipeline_cache(create_info, None) } {
        Ok(handle) => {
            let cache = PipelineCache { device, handle };

            #[cfg(debug_assertions)]
            if let Ok(debug_name) = CString::new(format!("graphics pipeline cache: {name}")) {
                vk_try!(
                    "name graphics pipeline cache",
                    cache.device.set_name(debug_name.as_c_str(), cache.handle),
                );
            }

            Ok(Some(cache))
        }
        Err(result) => {
            debug!("Cache creation for graphics pipeline '{name}' failed: {result:?}");
            Ok(None)
        }
    }
}

fn pipeline_cache_path(name: &str) -> PathBuf {
    Path::new(PIPELINE_CACHE_DIR).join(format!("{name}.pcache"))
}

fn is_valid_pipeline_name(name: &str) -> bool {
    !name.is_empty()
        && !name.chars().any(|ch| ch.is_whitespace() || ch == '/' || ch == '\\' || ch == '\0')
}

fn load_compatible_pipeline_cache(
    path: &Path,
    properties: &vk::PhysicalDeviceProperties,
) -> Option<Vec<u8>> {
    match fs::read(path) {
        Ok(cache_data) => compatible_pipeline_cache(cache_data, properties).or_else(|| {
            debug!("Ignoring incompatible pipeline cache `{}`", path.display());
            None
        }),
        Err(err) if err.kind() == io::ErrorKind::NotFound => None,
        Err(err) => {
            debug!("Ignoring unreadable pipeline cache `{}`: {err}", path.display());
            None
        }
    }
}

fn compatible_pipeline_cache(
    cache_data: Vec<u8>,
    properties: &vk::PhysicalDeviceProperties,
) -> Option<Vec<u8>> {
    let header_size = mem::size_of::<vk::PipelineCacheHeaderVersionOne>();
    if cache_data.len() < header_size {
        return None;
    }

    // SAFETY: `cache_data` has at least one full `PipelineCacheHeaderVersionOne` worth of bytes.
    // `read_unaligned` avoids imposing alignment requirements on bytes read from disk.
    let header =
        unsafe { cache_data.as_ptr().cast::<vk::PipelineCacheHeaderVersionOne>().read_unaligned() };

    let header_size_matches =
        usize::try_from(header.header_size).is_ok_and(|size| size >= header_size);

    (header_size_matches
        && header.header_version == vk::PipelineCacheHeaderVersion::ONE
        && header.vendor_id == properties.vendor_id
        && header.device_id == properties.device_id
        && header.pipeline_cache_uuid == properties.pipeline_cache_uuid)
        .then_some(cache_data)
}

fn write_pipeline_cache(
    path: &Path,
    cache_data: &[u8],
) -> core::result::Result<(), VulkanPipelineError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| {
            VulkanPipelineError::CacheDirectoryCreation { path: parent.to_owned(), source }
        })?;
    }

    fs::write(path, cache_data)
        .map_err(|source| VulkanPipelineError::CacheWrite { path: path.to_owned(), source })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_name_rejects_cache_unsafe_names() {
        assert!(is_valid_pipeline_name("terrain"));
        assert!(!is_valid_pipeline_name(""));
        assert!(!is_valid_pipeline_name("terrain main"));
        assert!(!is_valid_pipeline_name("../terrain"));
        assert!(!is_valid_pipeline_name("terrain\\main"));
    }

    #[test]
    fn pipeline_cache_accepts_matching_header() {
        let properties = pipeline_cache_properties(1, 2, [3; vk::UUID_SIZE]);
        let header = pipeline_cache_header(&properties);

        assert!(compatible_pipeline_cache(header_bytes(&header), &properties).is_some());
    }

    #[test]
    fn pipeline_cache_rejects_mismatched_header() {
        let properties = pipeline_cache_properties(1, 2, [3; vk::UUID_SIZE]);
        let other_properties = pipeline_cache_properties(4, 2, [3; vk::UUID_SIZE]);
        let header = pipeline_cache_header(&other_properties);

        assert!(compatible_pipeline_cache(header_bytes(&header), &properties).is_none());
    }

    fn pipeline_cache_properties(
        vendor_id: u32,
        device_id: u32,
        pipeline_cache_uuid: [u8; vk::UUID_SIZE],
    ) -> vk::PhysicalDeviceProperties {
        vk::PhysicalDeviceProperties {
            vendor_id,
            device_id,
            pipeline_cache_uuid,
            ..Default::default()
        }
    }

    fn pipeline_cache_header(
        properties: &vk::PhysicalDeviceProperties,
    ) -> vk::PipelineCacheHeaderVersionOne {
        vk::PipelineCacheHeaderVersionOne {
            header_size: u32::try_from(mem::size_of::<vk::PipelineCacheHeaderVersionOne>())
                .unwrap_or(u32::MAX),
            header_version: vk::PipelineCacheHeaderVersion::ONE,
            vendor_id: properties.vendor_id,
            device_id: properties.device_id,
            pipeline_cache_uuid: properties.pipeline_cache_uuid,
        }
    }

    fn header_bytes(header: &vk::PipelineCacheHeaderVersionOne) -> Vec<u8> {
        // SAFETY: `header` is a live plain-old-data Vulkan header. The resulting slice is used
        // immediately to copy the bytes into an owned `Vec<u8>`.
        unsafe {
            std::slice::from_raw_parts(
                std::ptr::from_ref(header).cast::<u8>(),
                mem::size_of::<vk::PipelineCacheHeaderVersionOne>(),
            )
            .to_vec()
        }
    }
}
