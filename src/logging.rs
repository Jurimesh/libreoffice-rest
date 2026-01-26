use std::env;
use std::io;

use tracing::Level;
use tracing_subscriber::fmt::format::JsonFields;
use tracing_subscriber::fmt::{self};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

/// Log format to use
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LogFormat {
    /// Human-readable format for local development
    #[default]
    Pretty,
    /// Standard JSON format
    Json,
    /// GCP Cloud Logging compatible JSON format (severity instead of level)
    Gcp,
}

impl LogFormat {
    /// Parse from environment variable LOG_FORMAT
    pub fn from_env() -> Self {
        match env::var("LOG_FORMAT")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "json" => LogFormat::Json,
            "gcp" => LogFormat::Gcp,
            _ => LogFormat::Pretty,
        }
    }
}

/// Maps tracing log levels to GCP severity levels
fn level_to_gcp_severity(level: &Level) -> &'static str {
    match *level {
        Level::ERROR => "ERROR",
        Level::WARN => "WARNING",
        Level::INFO => "INFO",
        Level::DEBUG => "DEBUG",
        Level::TRACE => "DEBUG",
    }
}

/// Custom event formatter for GCP Cloud Logging format
/// Outputs JSON with `severity` field instead of `level` for GCP compatibility
struct GcpJsonFormat;

impl<S, N> fmt::FormatEvent<S, N> for GcpJsonFormat
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    N: for<'a> fmt::FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &fmt::FmtContext<'_, S, N>,
        mut writer: fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        use chrono::Utc;
        use std::fmt::Write;

        let metadata = event.metadata();
        let level = metadata.level();
        let severity = level_to_gcp_severity(level);
        let timestamp = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);

        // Build JSON object
        let mut json = String::with_capacity(256);
        write!(
            json,
            "{{\"severity\":\"{}\",\"time\":\"{}\"",
            severity, timestamp
        )?;

        // Add target
        write!(json, ",\"target\":\"{}\"", metadata.target())?;

        // Add source location for GCP
        if let (Some(file), Some(line)) = (metadata.file(), metadata.line()) {
            write!(
                json,
                ",\"logging.googleapis.com/sourceLocation\":{{\"file\":\"{}\",\"line\":{}}}",
                file, line
            )?;
        }

        // Add span context if available
        if let Some(scope) = ctx.event_scope() {
            let spans: Vec<&str> = scope.map(|s| s.name()).collect();
            if !spans.is_empty() {
                let span_path: Vec<&str> = spans.iter().rev().copied().collect();
                write!(json, ",\"span\":\"{}\"", span_path.join(" > "))?;
            }
        }

        // Format event fields
        let mut visitor = JsonFieldVisitor::new();
        event.record(&mut visitor);

        if !visitor.fields.is_empty() {
            for (key, value) in &visitor.fields {
                write!(json, ",\"{}\":{}", key, value)?;
            }
        }

        // Close JSON object
        json.push('}');

        writeln!(writer, "{}", json)?;
        Ok(())
    }
}

/// Visitor to collect event fields as JSON values
struct JsonFieldVisitor {
    fields: Vec<(String, String)>,
}

impl JsonFieldVisitor {
    fn new() -> Self {
        Self { fields: Vec::new() }
    }
}

impl tracing::field::Visit for JsonFieldVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let escaped = escape_json_string(value);
        self.fields
            .push((field.name().to_string(), format!("\"{}\"", escaped)));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let debug_str = format!("{:?}", value);
        let escaped = escape_json_string(&debug_str);
        self.fields
            .push((field.name().to_string(), format!("\"{}\"", escaped)));
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        let error_str = value.to_string();
        let escaped = escape_json_string(&error_str);
        self.fields
            .push((field.name().to_string(), format!("\"{}\"", escaped)));
    }
}

/// Escape special characters for JSON strings
fn escape_json_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => result.push(c),
        }
    }
    result
}

/// Initialize tracing with the specified format.
///
/// Environment variables:
/// - `LOG_FORMAT`: "pretty" (default), "json", or "gcp"
/// - `RUST_LOG`: Log filter (default: "info")
/// - `DISABLE_ANSI_LOGGING`: Set to disable ANSI colors in pretty format
pub fn setup_tracing(format: LogFormat) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    match format {
        LogFormat::Pretty => {
            let use_ansi = env::var("DISABLE_ANSI_LOGGING")
                .map(|v| v.is_empty())
                .unwrap_or_else(|_| io::IsTerminal::is_terminal(&io::stdout()));

            Registry::default()
                .with(filter)
                .with(
                    fmt::layer()
                        .with_thread_ids(true)
                        .with_target(true)
                        .with_ansi(use_ansi),
                )
                .init();
        }
        LogFormat::Json => {
            Registry::default()
                .with(filter)
                .with(fmt::layer().json())
                .init();
        }
        LogFormat::Gcp => {
            let gcp_layer = fmt::layer()
                .event_format(GcpJsonFormat)
                .fmt_fields(JsonFields::new());

            Registry::default().with(filter).with(gcp_layer).init();
        }
    }
}

/// Initialize tracing using LOG_FORMAT environment variable
pub fn setup_tracing_from_env() {
    setup_tracing(LogFormat::from_env());
}
