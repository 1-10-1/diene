//! Top-level facade for the Diene game engine.
//!
//! This crate is the project-level entry point for applications and documentation.
//! It re-exports the default runtime API at the crate root and groups lower-level
//! engine interfaces into focused modules.
//!
//! # Quick Start
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let app = diene::Application::builder().with_name("Diene Sandbox").build()?;
//!
//! app.run()?;
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]

pub use self::app::{Application, ApplicationBuilder, ApplicationError, RendererBackend};

/// Application construction, runtime policy, and host lifecycle APIs.
pub mod app {
    pub use engine_core::app::{
        ApplicationHost, ApplicationHostBuildError, ApplicationHostError, WindowError,
    };
    pub use engine_runtime::{Application, ApplicationBuilder, ApplicationError, RendererBackend};
}

/// Commonly used application-facing types.
pub mod prelude {
    pub use crate::{Application, ApplicationBuilder, ApplicationError, RendererBackend};
}

/// Renderer abstraction shared by engine orchestration and backends.
pub mod renderer {
    pub use engine_renderer_api::{
        BoxedRenderer, BoxedRendererFactory, DisplayHandle, HandleError, HasDisplayHandle,
        HasWindowHandle, RenderExtent, RenderWindow, Renderer, RendererError, RendererFactory,
        WindowHandle,
    };
}
