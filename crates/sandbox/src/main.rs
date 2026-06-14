//! Development sandbox for running and testing the engine.

use anyhow::{Result, anyhow};

fn main() -> Result<()> {
    let err = sandbox::run();

    match err {
        Ok(()) => Ok(()),
        Err(e) => {
            // e.to_string() captures only the top-level message without the "caused by" trace
            Err(anyhow!("{e}"))
        }
    }
}
