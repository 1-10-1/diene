use std::{fmt as std_fmt, fs::File, time::Instant};

use anyhow::{Context, Result};
use tracing::{Event, Level, Subscriber};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    EnvFilter,
    fmt::{
        self, FmtContext,
        format::{FormatEvent, FormatFields, Writer},
    },
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
};

/// Keeps the async file logger alive.
///
/// Drop this only when the program is shutting down.
#[derive(Debug)]
pub struct LoggerGuard {
    _file_guard: WorkerGuard,
}

#[derive(Debug, Clone)]
struct EngineFormatter {
    debug: bool,
    started_at: Instant,
}

impl EngineFormatter {
    fn new() -> Self {
        Self {
            debug: cfg!(debug_assertions),
            started_at: Instant::now(),
        }
    }
}

impl<S, N> FormatEvent<S, N> for EngineFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std_fmt::Result {
        let metadata = event.metadata();

        write_level(&mut writer, *metadata.level())?;

        let elapsed_ms = self.started_at.elapsed().as_millis();
        write!(writer, " [{elapsed_ms}ms]")?;

        if self.debug {
            let file = metadata.file().unwrap_or("<unknown>");
            let line = metadata
                .line()
                .map_or_else(|| "?".to_owned(), |line| line.to_string());

            write!(writer, " [{file}:{line} {}]", metadata.target())?;
        }

        writeln!(writer)?;
        write!(writer, "-> ")?;

        ctx.field_format().format_fields(writer.by_ref(), event)?;

        writeln!(writer)
    }
}

fn write_level(writer: &mut Writer<'_>, level: Level) -> std_fmt::Result {
    let label = match level {
        Level::TRACE => "󰍉 TRACE",
        Level::DEBUG => "󰆧 DEBUG",
        Level::INFO => "󰋼 INFO",
        Level::WARN => "󰀦 WARN",
        Level::ERROR => "󰅚 ERROR",
    };

    if writer.has_ansi_escapes() {
        let color = match level {
            Level::TRACE => "\x1b[90m",
            Level::DEBUG => "\x1b[34m",
            Level::INFO => "\x1b[32m",
            Level::WARN => "\x1b[33m",
            Level::ERROR => "\x1b[31;1m",
        };

        write!(writer, "{color}[{label}]\x1b[0m")
    } else {
        write!(writer, "[{label}]")
    }
}

/// Initializes the logger.
pub fn init() -> Result<LoggerGuard> {
    let log_path = std::env::current_dir()
        .context("failed to get current working directory")?
        .join("minecraft.log");

    let log_file = File::create(&log_path).with_context(|| {
        format!("failed to create log file at {}", log_path.display())
    })?;

    let formatter = EngineFormatter::new();

    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .event_format(formatter.clone());

    let (file_writer, file_guard) = tracing_appender::non_blocking(log_file);

    let file_layer = fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .event_format(formatter);

    let default_level = if cfg!(debug_assertions) {
        "trace"
    } else {
        "info"
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::registry()
        .with(filter)
        .with(stdout_layer)
        .with(file_layer)
        .try_init()
        .context("failed to initialize global tracing subscriber")?;

    Ok(LoggerGuard { _file_guard: file_guard })
}
