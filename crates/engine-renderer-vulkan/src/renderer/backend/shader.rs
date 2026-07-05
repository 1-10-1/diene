#![allow(dead_code)]

use std::{
    ffi::{CString, NulError},
    io,
    path::PathBuf,
    sync::Arc,
};

use ash::vk;
use shader_slang as slang;
use slang::Downcast;
use thiserror::Error;

use crate::renderer::backend::{call_error::VulkanCallError, device::VulkanLogicalDevice};

const SHADER_SEARCH_PATH: &str = "shaders";
const SPIRV_TARGET: i64 = 0;
const SPIRV_ASSEMBLY_TARGET: i64 = 1;

/// Errors returned by shader compilation and module creation.
#[derive(Debug, Error)]
pub(super) enum VulkanShaderError {
    /// Vulkan API call returned an error value.
    #[error(transparent)]
    UnexpectedResult(#[from] VulkanCallError),

    /// Slang global session creation failed.
    #[error("failed to create Slang global session")]
    GlobalSessionCreation,

    /// Slang compiler session creation failed.
    #[error("failed to create Slang compiler session")]
    SessionCreation,

    /// Shader module name contained an interior NUL byte.
    #[error("shader module name contained an interior NUL byte")]
    InvalidModuleName {
        /// Underlying string conversion error.
        #[source]
        source: NulError,
    },

    /// Shader entrypoint name contained an interior NUL byte.
    #[error("shader entrypoint `{entrypoint}` contained an interior NUL byte")]
    InvalidEntrypointName {
        /// Entrypoint name that could not be represented as a C string.
        entrypoint: String,

        /// Underlying string conversion error.
        #[source]
        source: NulError,
    },

    /// Shader build request did not contain any entrypoints.
    #[error("shader module `{module}` did not specify any entrypoints")]
    NoEntrypoints {
        /// Slang module name.
        module: String,
    },

    /// Slang module compilation failed.
    #[error("failed to compile Slang module `{module}`: {source}")]
    ModuleCompilationFailed {
        /// Slang module name.
        module: String,

        /// Slang diagnostic error.
        #[source]
        source: slang::Error,
    },

    /// Slang entrypoint lookup failed.
    #[error("Slang entrypoint `{entrypoint}` was not found in module `{module}`")]
    InaccessibleEntrypoint {
        /// Slang module name.
        module: String,

        /// Missing entrypoint name.
        entrypoint: String,
    },

    /// Slang component composition failed.
    #[error("failed to compose Slang shader program `{module}`: {source}")]
    CompositionFailure {
        /// Slang module name.
        module: String,

        /// Slang diagnostic error.
        #[source]
        source: slang::Error,
    },

    /// Slang program linking failed.
    #[error("failed to link Slang shader program `{module}`: {source}")]
    LinkFailure {
        /// Slang module name.
        module: String,

        /// Slang diagnostic error.
        #[source]
        source: slang::Error,
    },

    /// Slang target code generation failed.
    #[error("failed to generate target code for Slang shader program `{module}`: {source}")]
    TargetCodeGenerationFailure {
        /// Slang module name.
        module: String,

        /// Slang diagnostic error.
        #[source]
        source: slang::Error,
    },

    /// Failed to write generated SPIR-V assembly for debugging.
    #[error("failed to write generated SPIR-V assembly to `{path}`: {source}")]
    AssemblyWriteFailure {
        /// Assembly output path.
        path: PathBuf,

        /// Underlying filesystem error.
        #[source]
        source: io::Error,
    },

    /// Slang returned malformed SPIR-V bytecode.
    #[error("Slang returned SPIR-V bytecode for module `{module}` with invalid byte length {len}")]
    InvalidSpirvBytecode {
        /// Slang module name.
        module: String,

        /// Byte length returned by Slang.
        len: usize,
    },
}

/// Shader entrypoint and Vulkan stage metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ShaderEntrypoint {
    name: String,
    stage: vk::ShaderStageFlags,
}

impl ShaderEntrypoint {
    /// Creates an entrypoint description for a Slang module.
    pub(super) fn new(name: impl Into<String>, stage: vk::ShaderStageFlags) -> Self {
        Self { name: name.into(), stage }
    }
}

impl From<(&str, vk::ShaderStageFlags)> for ShaderEntrypoint {
    fn from((name, stage): (&str, vk::ShaderStageFlags)) -> Self {
        Self::new(name, stage)
    }
}

impl From<(String, vk::ShaderStageFlags)> for ShaderEntrypoint {
    fn from((name, stage): (String, vk::ShaderStageFlags)) -> Self {
        Self::new(name, stage)
    }
}

#[derive(Debug)]
struct ShaderStage {
    name: CString,
    stage: vk::ShaderStageFlags,
}

/// Compiles Slang shader modules into Vulkan shader modules.
pub(super) struct VulkanShaderManager {
    device: Arc<VulkanLogicalDevice>,
    session: slang::Session,
}

impl std::fmt::Debug for VulkanShaderManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VulkanShaderManager").finish_non_exhaustive()
    }
}

impl VulkanShaderManager {
    /// Creates the global Slang compiler session used to create shader managers.
    pub(super) fn make_global_session()
    -> core::result::Result<slang::GlobalSession, VulkanShaderError> {
        slang::GlobalSession::new().ok_or(VulkanShaderError::GlobalSessionCreation)
    }

    /// Creates a shader manager for the provided logical device.
    pub(super) fn new(
        device: Arc<VulkanLogicalDevice>,
        global_session: &slang::GlobalSession,
    ) -> core::result::Result<Self, VulkanShaderError> {
        let spirv_profile = global_session.find_profile("spirv_1_5");
        let targets = [
            slang::TargetDesc::default().format(slang::CompileTarget::Spirv).profile(spirv_profile),
            slang::TargetDesc::default()
                .format(slang::CompileTarget::SpirvAsm)
                .profile(spirv_profile),
        ];
        let search_path = CString::new(SHADER_SEARCH_PATH)
            .map_err(|source| VulkanShaderError::InvalidModuleName { source })?;
        let search_paths = [search_path.as_ptr()];
        let options = slang::CompilerOptions::default()
            .language(slang::SourceLanguage::Slang)
            .optimization(slang::OptimizationLevel::Maximal)
            .debug_information(slang::DebugInfoLevel::Maximal)
            .matrix_layout_column(true);
        let session_desc = slang::SessionDesc::default()
            .targets(&targets)
            .search_paths(&search_paths)
            .options(&options);
        let session = global_session
            .create_session(&session_desc)
            .ok_or(VulkanShaderError::SessionCreation)?;

        Ok(Self { device, session })
    }

    /// Builds a Slang module with the requested entrypoints.
    pub(super) fn build<I, E>(
        &self,
        module_name: &str,
        entrypoints: I,
    ) -> core::result::Result<VulkanShader, VulkanShaderError>
    where
        I: IntoIterator<Item = E>,
        E: Into<ShaderEntrypoint>,
    {
        CString::new(module_name)
            .map_err(|source| VulkanShaderError::InvalidModuleName { source })?;

        let module_name = module_name.to_owned();
        let entrypoints = entrypoints.into_iter().map(Into::into).collect::<Vec<_>>();

        if entrypoints.is_empty() {
            return Err(VulkanShaderError::NoEntrypoints { module: module_name });
        }

        let module = self.session.load_module(&module_name).map_err(|source| {
            VulkanShaderError::ModuleCompilationFailed { module: module_name.clone(), source }
        })?;

        let mut slang_entrypoints = Vec::with_capacity(entrypoints.len());
        let mut stages = Vec::with_capacity(entrypoints.len());

        for entrypoint in entrypoints {
            let entrypoint_name = CString::new(entrypoint.name.clone()).map_err(|source| {
                VulkanShaderError::InvalidEntrypointName {
                    entrypoint: entrypoint.name.clone(),
                    source,
                }
            })?;
            let slang_entrypoint =
                module.find_entry_point_by_name(&entrypoint.name).ok_or_else(|| {
                    VulkanShaderError::InaccessibleEntrypoint {
                        module: module_name.clone(),
                        entrypoint: entrypoint.name.clone(),
                    }
                })?;

            slang_entrypoints.push(slang_entrypoint);
            stages.push(ShaderStage { name: entrypoint_name, stage: entrypoint.stage });
        }

        let component_types = std::iter::once(module.downcast().clone())
            .chain(slang_entrypoints.iter().map(|entrypoint| entrypoint.downcast().clone()))
            .collect::<Vec<slang::ComponentType>>();

        let program =
            self.session.create_composite_component_type(&component_types).map_err(|source| {
                VulkanShaderError::CompositionFailure { module: module_name.clone(), source }
            })?;
        let linked_program = program.link().map_err(|source| VulkanShaderError::LinkFailure {
            module: module_name.clone(),
            source,
        })?;

        let spirv = linked_program.target_code(SPIRV_TARGET).map_err(|source| {
            VulkanShaderError::TargetCodeGenerationFailure { module: module_name.clone(), source }
        })?;
        let assembly = linked_program.target_code(SPIRV_ASSEMBLY_TARGET).map_err(|source| {
            VulkanShaderError::TargetCodeGenerationFailure { module: module_name.clone(), source }
        })?;

        let assembly_path =
            PathBuf::from(SHADER_SEARCH_PATH).join(format!("{module_name}.slang.asm"));
        std::fs::write(&assembly_path, assembly.as_slice()).map_err(|source| {
            VulkanShaderError::AssemblyWriteFailure { path: assembly_path, source }
        })?;

        let spirv_words = spirv_words(&module_name, spirv.as_slice())?;
        let create_info = vk::ShaderModuleCreateInfo::default().code(&spirv_words);

        // SAFETY: `create_info` points to SPIR-V words owned by this stack frame and valid through
        // the duration of the call. No custom allocator is used.
        let module = vk_try!("create shader module", unsafe {
            self.device.get_handle().create_shader_module(&create_info, None)
        });

        #[cfg(debug_assertions)]
        if let Ok(name) = CString::new(format!("Shader Module: {module_name}")) {
            vk_try!("name shader module", self.device.set_name(name.as_c_str(), module));
        }

        Ok(VulkanShader { device: self.device.clone(), module, stages })
    }
}

/// Owns a Vulkan shader module and its pipeline stage metadata.
pub(super) struct VulkanShader {
    device: Arc<VulkanLogicalDevice>,
    module: vk::ShaderModule,
    stages: Vec<ShaderStage>,
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

    /// Builds pipeline shader stage infos that borrow this shader's entrypoint names.
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
        // SAFETY: `self.module` was created through `self.device` and is destroyed exactly once.
        unsafe {
            self.device.get_handle().destroy_shader_module(self.module, None);
        }
    }
}

fn spirv_words(module: &str, bytes: &[u8]) -> core::result::Result<Vec<u32>, VulkanShaderError> {
    let (words, remainder) = bytes.as_chunks::<4>();

    if !remainder.is_empty() {
        return Err(VulkanShaderError::InvalidSpirvBytecode {
            module: module.to_owned(),
            len: bytes.len(),
        });
    }

    Ok(words.iter().map(|word| u32::from_le_bytes(*word)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_spirv_bytes_to_words() {
        assert_eq!(spirv_words("test", &[1, 0, 0, 0, 2, 0, 0, 0]).ok(), Some(vec![1, 2]));
    }

    #[test]
    fn rejects_spirv_byte_count_not_aligned_to_word_size() {
        assert!(matches!(
            spirv_words("test", &[1, 2, 3]),
            Err(VulkanShaderError::InvalidSpirvBytecode { len: 3, .. })
        ));
    }
}
