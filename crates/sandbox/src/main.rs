//! Development sandbox for running and testing the engine.

use engine_core::app::Application;

fn main() -> anyhow::Result<()> {
    let _logger_guard = common::logging::init()?;

    let mut app = Application::builder().with_name("Diene Sandbox").build();

    app.tick();

    Ok(())
}
