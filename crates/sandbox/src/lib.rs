#![allow(missing_docs)]
#![forbid(unsafe_code)]

use common::logging::macros::*;
use engine_runtime::Application;

pub fn run() -> anyhow::Result<()> {
    let _logger_guard = common::logging::init()?;

    let app_name = "diene sandbox";

    let app = Application::builder().with_name(app_name).build()?;

    app.run()?;

    info!("[{}] sandbox application exited", app_name);

    Ok(())
}
