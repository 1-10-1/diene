//! Development sandbox for running and testing the engine.

use std::fmt::Write;

use common::logging::macros::*;

fn main() {
    if let Err(err) = sandbox::run() {
        print_error(&err);
    }
}

fn print_error(err: &anyhow::Error) {
    let mut chain = err.chain();

    let mut buffer = String::new();

    if let Some(error) = chain.next() {
        let _ = writeln!(buffer, "ERROR: {}", local_error_message(error));
    }

    let mut indent = 4;

    for cause in chain {
        let _ = writeln!(buffer, "{:indent$}because: {}", "", local_error_message(cause));
        indent += 4;
    }

    error!("{buffer}");
}

fn local_error_message(error: &(dyn std::error::Error + 'static)) -> String {
    let message = error.to_string();

    let Some(source) = error.source() else {
        return message;
    };

    let source_suffix = format!(": {source}");

    message.strip_suffix(&source_suffix).unwrap_or(&message).to_owned()
}
