//! Logging setup. The arguments match those that are used by Gazette, flowctl and the other
//! Flow CLIs, so that there's consistency across all of them.

use std::fmt;
use std::io::IsTerminal;

#[derive(Debug, clap::Args)]
pub struct LogArgs {
    /// The log verbosity. Can be one of trace|debug|info|warn|error|off
    #[arg(
        long = "log.level",
        default_value_t = LogLevel::Warn,
        group = "logging",
        ignore_case = true,
        value_enum,
        global = true
    )]
    pub level: LogLevel,

    #[arg(long = "log.format", value_enum, global = true, group = "logging")]
    pub format: Option<LogFormat>,
}

#[derive(Debug, clap::ValueEnum, Clone, Copy)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        })
    }
}

/// The format for logs.
#[derive(Debug, clap::ValueEnum, Clone, Copy)]
pub enum LogFormat {
    /// Logs are written to stderr in jsonl format. This format is very compatible with Flow's log
    /// parsing, which means that they will get forwarded with the proper level and will retain the
    /// the structure of fields.
    Json,
    /// Plain text with no colors.
    Text,
    /// Same as plain text, but with fancy colors for better readability in interactive terminals.
    Color,
}

fn default_log_format() -> LogFormat {
    if std::io::stderr().is_terminal() {
        LogFormat::Color
    } else {
        // If running non-interactively, default to JSON so that programatic users don't have to
        // always specify that.
        LogFormat::Json
    }
}

/// Initializes logging, using the given args. Panics if called twice.
pub fn init_logging(args: &LogArgs) {
    let builder = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(args.level.to_string())
        .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
        // Spans are only logged once they're closed, so there's a single log line per span
        // which carries its timing information and all of its recorded fields.
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_target(false);

    match args.format.unwrap_or_else(default_log_format) {
        LogFormat::Json => {
            builder
                .json()
                .flatten_event(true)
                .with_current_span(true)
                .with_span_list(false)
                .init();
        }
        LogFormat::Text => {
            builder.compact().with_ansi(false).init();
        }
        LogFormat::Color => {
            builder.compact().with_ansi(true).init();
        }
    }
}
