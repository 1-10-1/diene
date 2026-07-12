#![allow(dead_code)]

use std::{
    ffi::{CString, NulError},
    rc::Rc,
    sync::Arc,
};

use ash::vk;
use engine_shader::{
    CompiledShader, ShaderCompileError, ShaderCompiler, ShaderEntrypoint, ShaderStage,
};
use thiserror::Error;

use crate::renderer::backend::{call_error::VulkanCallError, device::VulkanLogicalDevice};

/// Errors returned by Vulkan shader module creation.
#[derive(Debug, Error)]
pub(super) enum VulkanShaderError {
    /// Shader compilation failed.
    #[error(transparent)]
    Compilation(#[from] ShaderCompileError),

    /// Vulkan API call returned an error value.
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),

    /// Shader entrypoint name contained an interior NUL byte.
    #[error("shader entrypoint `{entrypoint}` contained an interior NUL byte")]
    InvalidEntrypointName {
        /// Entrypoint name that could not be represented as a C
        /// string.
        entrypoint: String,

        /// Underlying string conversion error.
        #[source]
        source: NulError,
    },
}

#[derive(Debug)]
struct ShaderStageInfo {
    name: CString,
    stage: vk::ShaderStageFlags,
}

/// Creates Vulkan shader modules from backend-neutral compiled shader
/// artifacts.
pub(super) struct VulkanShaderManager {
    device: Arc<VulkanLogicalDevice>,
    compiler: Rc<ShaderCompiler>,
}

impl std::fmt::Debug for VulkanShaderManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanShaderManager").finish_non_exhaustive()
    }
}

impl VulkanShaderManager {
    /// Creates a shader manager for the provided logical device and
    /// compiler.
    pub(super) fn new(device: Arc<VulkanLogicalDevice>, compiler: Rc<ShaderCompiler>) -> Self {
        Self { device, compiler }
    }

    /// Compiles a shader module and creates a Vulkan shader module
    /// from it.
    pub(super) fn build<I, E>(
        &self,
        module_name: &str,
        entrypoints: I,
    ) -> core::result::Result<VulkanShader, VulkanShaderError>
    where
        I: IntoIterator<Item = E>,
        E: Into<ShaderEntrypoint>,
    {
        let compiled = self.compiler.compile(module_name, entrypoints)?;
        self.create_shader(&compiled)
    }

    /// Creates a Vulkan shader module from a precompiled shader
    /// artifact.
    pub(super) fn create_shader(
        &self,
        compiled: &CompiledShader,
    ) -> core::result::Result<VulkanShader, VulkanShaderError> {
        let stages = compiled
            .entrypoints()
            .iter()
            .map(|entrypoint| {
                Ok::<_, VulkanShaderError>(ShaderStageInfo {
                    name: CString::new(entrypoint.name()).map_err(|source| {
                        VulkanShaderError::InvalidEntrypointName {
                            entrypoint: entrypoint.name().to_owned(),
                            source,
                        }
                    })?,
                    stage: vulkan_stage(entrypoint.stage()),
                })
            })
            .collect::<core::result::Result<Vec<_>, _>>()?;

        let create_info = vk::ShaderModuleCreateInfo::default().code(compiled.spirv_words());

        // SAFETY: `create_info` points to SPIR-V words owned by `compiled`
        // and valid through the duration of the call. No custom
        // allocator is used.
        let module = vk_try!("create shader module", unsafe {
            self.device.handle().create_shader_module(&create_info, None)
        });

        #[cfg(debug_assertions)]
        if let Ok(name) = CString::new(format!("shader module: {}", compiled.module()))
            && let Err(result) = self.device.set_name(name.as_c_str(), module)
        {
            // SAFETY: `module` was just created from `self.device` and has not
            // been handed to an owner yet, so destroying it here
            // avoids leaking it on the error path.
            unsafe {
                self.device.handle().destroy_shader_module(module, None);
            }

            return Err(VulkanCallError::new("name shader module", result).into());
        }

        Ok(VulkanShader { device: self.device.clone(), module, stages })
    }
}

/// Owns a Vulkan shader module and its pipeline stage metadata.
pub(super) struct VulkanShader {
    device: Arc<VulkanLogicalDevice>,
    module: vk::ShaderModule,
    stages: Vec<ShaderStageInfo>,
}

impl std::fmt::Debug for VulkanShader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanShader")
            .field("module", &self.module)
            .field("stage_count", &self.stages.len())
            .finish_non_exhaustive()
    }
}

impl VulkanShader {
    /// Returns the underlying Vulkan shader module handle.
    pub(super) fn get(&self) -> vk::ShaderModule {
        self.module
    }

    /// Builds pipeline shader stage infos that borrow this shader's
    /// entrypoint names.
    pub(super) fn stage_infos(&self) -> Vec<vk::PipelineShaderStageCreateInfo<'_>> {
        self.stages
            .iter()
            .map(|entrypoint| {
                vk::PipelineShaderStageCreateInfo::default()
                    .stage(entrypoint.stage)
                    .module(self.module)
                    .name(entrypoint.name.as_c_str())
            })
            .collect()
    }
}

impl Drop for VulkanShader {
    fn drop(&mut self) {
        // SAFETY: `self.module` was created through `self.device` and is
        // destroyed exactly once.
        unsafe {
            self.device.handle().destroy_shader_module(self.module, None);
        }
    }
}

const fn vulkan_stage(stage: ShaderStage) -> vk::ShaderStageFlags {
    match stage {
        ShaderStage::Vertex => vk::ShaderStageFlags::VERTEX,
        ShaderStage::Fragment => vk::ShaderStageFlags::FRAGMENT,
        ShaderStage::Compute => vk::ShaderStageFlags::COMPUTE,
        ShaderStage::Geometry => vk::ShaderStageFlags::GEOMETRY,
        ShaderStage::TessellationControl => vk::ShaderStageFlags::TESSELLATION_CONTROL,
        ShaderStage::TessellationEvaluation => vk::ShaderStageFlags::TESSELLATION_EVALUATION,
    }
}
