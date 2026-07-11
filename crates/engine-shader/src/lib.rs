//! Backend-agnostic shader compilation.

#![forbid(unsafe_code)]

use std::{
    ffi::{CString, NulError},
    io,
    path::{Path, PathBuf},
};

use shader_slang as slang;
use slang::Downcast;
use thiserror::Error;

const DEFAULT_SEARCH_PATH: &str = "shaders";
const DEFAULT_SPIRV_PROFILE: &str = "spirv_1_5";
const SPIRV_TARGET: i64 = 0;
const SPIRV_ASSEMBLY_TARGET: i64 = 1;

/// Shader compiler options shared by renderer backends.
#[derive(Clone, Debug)]
pub struct ShaderCompilerOptions {
    search_paths: Vec<PathBuf>,
    assembly_output_dir: Option<PathBuf>,
    spirv_profile: String,
}

impl Default for ShaderCompilerOptions {
    fn default() -> Self {
        Self {
            search_paths: vec![PathBuf::from(DEFAULT_SEARCH_PATH)],
            assembly_output_dir: Some(PathBuf::from(DEFAULT_SEARCH_PATH)),
            spirv_profile: DEFAULT_SPIRV_PROFILE.to_owned(),
        }
    }
}

impl ShaderCompilerOptions {
    /// Adds a search path used to resolve shader modules and includes.
    #[must_use]
    pub fn with_search_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.search_paths.push(path.into());
        self
    }

    /// Replaces the search paths used to resolve shader modules and includes.
    #[must_use]
    pub fn with_search_paths<I, P>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.search_paths = paths.into_iter().map(Into::into).collect();
        self
    }

    /// Sets where generated SPIR-V assembly is written.
    #[must_use]
    pub fn with_assembly_output_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.assembly_output_dir = Some(path.into());
        self
    }

    /// Disables writing generated SPIR-V assembly to disk.
    #[must_use]
    pub fn without_assembly_output(mut self) -> Self {
        self.assembly_output_dir = None;
        self
    }

    /// Sets the Slang profile used for generated SPIR-V targets.
    #[must_use]
    pub fn with_spirv_profile(mut self, profile: impl Into<String>) -> Self {
        self.spirv_profile = profile.into();
        self
    }
}

/// Errors returned by shader compilation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ShaderCompileError {
    /// Slang global session creation failed.
    #[error("failed to create Slang global session")]
    GlobalSessionCreation,

    /// Slang compiler session creation failed.
    #[error("failed to create Slang compiler session")]
    SessionCreation,

    /// A search path contained an interior NUL byte.
    #[error("shader search path `{path}` contained an interior NUL byte")]
    InvalidSearchPath {
        /// Search path that could not be represented as a C string.
        path: PathBuf,

        /// Underlying string conversion error.
        #[source]
        source: NulError,
    },

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
    #[error("failed to compile Slang module `{module}`")]
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

/// Backend-neutral shader stage.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ShaderStage {
    /// Vertex shader stage.
    Vertex,

    /// Fragment/pixel shader stage.
    Fragment,

    /// Compute shader stage.
    Compute,

    /// Geometry shader stage.
    Geometry,

    /// Tessellation control/hull shader stage.
    TessellationControl,

    /// Tessellation evaluation/domain shader stage.
    TessellationEvaluation,
}

/// Shader entrypoint requested from a module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShaderEntrypoint {
    name: String,
    stage: ShaderStage,
}

impl ShaderEntrypoint {
    /// Creates an entrypoint description for a shader module.
    pub fn new(name: impl Into<String>, stage: ShaderStage) -> Self {
        Self { name: name.into(), stage }
    }

    /// Returns the entrypoint function name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the backend-neutral shader stage.
    pub const fn stage(&self) -> ShaderStage {
        self.stage
    }
}

impl From<(&str, ShaderStage)> for ShaderEntrypoint {
    fn from((name, stage): (&str, ShaderStage)) -> Self {
        Self::new(name, stage)
    }
}

impl From<(String, ShaderStage)> for ShaderEntrypoint {
    fn from((name, stage): (String, ShaderStage)) -> Self {
        Self::new(name, stage)
    }
}

/// Compiled shader entrypoint metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledEntrypoint {
    name: String,
    stage: ShaderStage,
}

impl CompiledEntrypoint {
    /// Returns the entrypoint function name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the backend-neutral shader stage.
    pub const fn stage(&self) -> ShaderStage {
        self.stage
    }
}

/// Backend-neutral compiled shader artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledShader {
    module: String,
    spirv_words: Vec<u32>,
    spirv_assembly: Vec<u8>,
    entrypoints: Vec<CompiledEntrypoint>,
}

impl CompiledShader {
    /// Returns the Slang module name.
    pub fn module(&self) -> &str {
        &self.module
    }

    /// Returns generated SPIR-V words.
    pub fn spirv_words(&self) -> &[u32] {
        &self.spirv_words
    }

    /// Returns generated SPIR-V assembly bytes.
    pub fn spirv_assembly(&self) -> &[u8] {
        &self.spirv_assembly
    }

    /// Returns compiled entrypoint metadata.
    pub fn entrypoints(&self) -> &[CompiledEntrypoint] {
        &self.entrypoints
    }
}

/// Compiles Slang shader modules into backend-neutral SPIR-V artifacts.
pub struct ShaderCompiler {
    session: slang::Session,
    _global_session: slang::GlobalSession,
    options: ShaderCompilerOptions,
}

impl std::fmt::Debug for ShaderCompiler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShaderCompiler").field("options", &self.options).finish_non_exhaustive()
    }
}

impl ShaderCompiler {
    /// Creates a Slang shader compiler with default options.
    pub fn new() -> core::result::Result<Self, ShaderCompileError> {
        Self::with_options(ShaderCompilerOptions::default())
    }

    /// Creates a Slang shader compiler with custom options.
    pub fn with_options(
        options: ShaderCompilerOptions,
    ) -> core::result::Result<Self, ShaderCompileError> {
        let global_session =
            slang::GlobalSession::new().ok_or(ShaderCompileError::GlobalSessionCreation)?;

        let spirv_profile = global_session.find_profile(&options.spirv_profile);

        let targets = [
            slang::TargetDesc::default().format(slang::CompileTarget::Spirv).profile(spirv_profile),
            slang::TargetDesc::default()
                .format(slang::CompileTarget::SpirvAsm)
                .profile(spirv_profile),
        ];

        let search_paths = options
            .search_paths
            .iter()
            .map(|path| path_to_cstring(path))
            .collect::<core::result::Result<Vec<_>, _>>()?;

        let search_path_ptrs = search_paths.iter().map(|path| path.as_ptr()).collect::<Vec<_>>();

        let compiler_options = slang::CompilerOptions::default()
            .language(slang::SourceLanguage::Slang)
            .optimization(slang::OptimizationLevel::Maximal)
            .debug_information(slang::DebugInfoLevel::Maximal)
            .matrix_layout_column(true);

        let session_desc = slang::SessionDesc::default()
            .targets(&targets)
            .search_paths(&search_path_ptrs)
            .options(&compiler_options);

        let session = global_session
            .create_session(&session_desc)
            .ok_or(ShaderCompileError::SessionCreation)?;

        Ok(Self { _global_session: global_session, session, options })
    }

    /// Compiles a Slang module with the requested entrypoints.
    pub fn compile<I, E>(
        &self,
        module_name: &str,
        entrypoints: I,
    ) -> core::result::Result<CompiledShader, ShaderCompileError>
    where
        I: IntoIterator<Item = E>,
        E: Into<ShaderEntrypoint>,
    {
        CString::new(module_name)
            .map_err(|source| ShaderCompileError::InvalidModuleName { source })?;

        let module_name = module_name.to_owned();
        let entrypoints = entrypoints.into_iter().map(Into::into).collect::<Vec<_>>();

        if entrypoints.is_empty() {
            return Err(ShaderCompileError::NoEntrypoints { module: module_name });
        }

        let module = self.session.load_module(&module_name).map_err(|source| {
            ShaderCompileError::ModuleCompilationFailed { module: module_name.clone(), source }
        })?;

        let mut slang_entrypoints = Vec::with_capacity(entrypoints.len());
        let mut compiled_entrypoints = Vec::with_capacity(entrypoints.len());

        for entrypoint in entrypoints {
            CString::new(entrypoint.name.clone()).map_err(|source| {
                ShaderCompileError::InvalidEntrypointName {
                    entrypoint: entrypoint.name.clone(),
                    source,
                }
            })?;
            let slang_entrypoint =
                module.find_entry_point_by_name(&entrypoint.name).ok_or_else(|| {
                    ShaderCompileError::InaccessibleEntrypoint {
                        module: module_name.clone(),
                        entrypoint: entrypoint.name.clone(),
                    }
                })?;

            slang_entrypoints.push(slang_entrypoint);
            compiled_entrypoints
                .push(CompiledEntrypoint { name: entrypoint.name, stage: entrypoint.stage });
        }

        let component_types = std::iter::once(module.downcast().clone())
            .chain(slang_entrypoints.iter().map(|entrypoint| entrypoint.downcast().clone()))
            .collect::<Vec<slang::ComponentType>>();

        let program =
            self.session.create_composite_component_type(&component_types).map_err(|source| {
                ShaderCompileError::CompositionFailure { module: module_name.clone(), source }
            })?;

        let linked_program = program.link().map_err(|source| ShaderCompileError::LinkFailure {
            module: module_name.clone(),
            source,
        })?;

        let spirv = linked_program.target_code(SPIRV_TARGET).map_err(|source| {
            ShaderCompileError::TargetCodeGenerationFailure { module: module_name.clone(), source }
        })?;

        let assembly = linked_program.target_code(SPIRV_ASSEMBLY_TARGET).map_err(|source| {
            ShaderCompileError::TargetCodeGenerationFailure { module: module_name.clone(), source }
        })?;

        let spirv_assembly = assembly.as_slice().to_vec();

        if let Some(output_dir) = &self.options.assembly_output_dir {
            let assembly_path = output_dir.join(format!("{module_name}.slang.asm"));

            std::fs::write(&assembly_path, &spirv_assembly).map_err(|source| {
                ShaderCompileError::AssemblyWriteFailure { path: assembly_path, source }
            })?;
        }

        let spirv_words = spirv_words(&module_name, spirv.as_slice())?;

        Ok(CompiledShader {
            module: module_name,
            spirv_words,
            spirv_assembly,
            entrypoints: compiled_entrypoints,
        })
    }
}

fn path_to_cstring(path: &Path) -> core::result::Result<CString, ShaderCompileError> {
    CString::new(path.to_string_lossy().as_bytes())
        .map_err(|source| ShaderCompileError::InvalidSearchPath { path: path.to_owned(), source })
}

fn spirv_words(module: &str, bytes: &[u8]) -> core::result::Result<Vec<u32>, ShaderCompileError> {
    let (words, remainder) = bytes.as_chunks::<4>();

    if !remainder.is_empty() {
        return Err(ShaderCompileError::InvalidSpirvBytecode {
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
            Err(ShaderCompileError::InvalidSpirvBytecode { len: 3, .. })
        ));
    }
}
