//! Sandbox executable for testing the engine during development.

use engine_core::app::Application;

fn main() {
    let mut app = Application::builder().with_name("Diene Sandbox").build();

    loop {
        app.tick();
    }
}
