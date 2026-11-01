//! Tracing subscriber layers (specs/rust-port/05 §6.3): `--debug` file layer,
//! opt-in OTLP export, tokio runtime tracing / optional tokio-console.
//! Nothing here ever writes to stdout — that belongs to LSP frames.

use anyhow::Result;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Keeps the OTLP provider alive; flushed on drop / `exit` (01 §3.3).
pub struct Guard {
    otlp: Option<opentelemetry_sdk::trace::TracerProvider>,
}

impl Guard {
    pub fn shutdown(&self) {
        if let Some(provider) = &self.otlp {
            let _ = provider.shutdown();
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.shutdown();
    }
}

static GUARD: std::sync::OnceLock<Guard> = std::sync::OnceLock::new();

/// Flush exporters before `process::exit` (dispatcher `exit` handler).
pub fn flush() {
    if let Some(guard) = GUARD.get() {
        guard.shutdown();
    }
}

pub fn init(debug: bool, otlp: Option<&str>) -> Result<()> {
    let default_level = if debug { "debug" } else { "info" };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    // `--debug` file layer: log.txt beside the executable (01 §5).
    let file_layer = if debug {
        let dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_default();
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("log.txt"))?;
        Some(
            tracing_subscriber::fmt::layer()
                .with_writer(file)
                .with_ansi(false)
                .with_target(true),
        )
    } else {
        None
    };

    // Opt-in OTLP layer (05 §6.3): --otlp[=endpoint] or the standard env var.
    let enabled_by_env = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok();
    let otlp_provider = match (otlp, enabled_by_env) {
        (None, false) => None,
        (endpoint, _) => Some(build_otlp(endpoint.filter(|e| !e.is_empty()))?),
    };
    let otlp_layer = otlp_provider.as_ref().map(|provider| {
        use opentelemetry::trace::TracerProvider as _;
        tracing_opentelemetry::layer().with_tracer(provider.tracer("dbt-language-server"))
    });

    #[cfg(feature = "tokio-console")]
    let console_layer = Some(console_subscriber::spawn());
    #[cfg(not(feature = "tokio-console"))]
    let console_layer: Option<tracing_subscriber::layer::Identity> = None;

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(otlp_layer)
        .with(console_layer)
        .init();

    let _ = GUARD.set(Guard {
        otlp: otlp_provider,
    });
    Ok(())
}

fn build_otlp(endpoint: Option<&str>) -> Result<opentelemetry_sdk::trace::TracerProvider> {
    use opentelemetry::KeyValue;
    use opentelemetry_otlp::WithExportConfig;

    let mut exporter = opentelemetry_otlp::SpanExporter::builder().with_tonic();
    if let Some(endpoint) = endpoint {
        exporter = exporter.with_endpoint(endpoint.to_string());
    }
    let exporter = exporter.build()?;

    let resource = opentelemetry_sdk::Resource::new(vec![
        KeyValue::new("service.name", "dbt-language-server"),
        KeyValue::new("service.version", crate::VERSION),
    ]);
    Ok(opentelemetry_sdk::trace::TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_resource(resource)
        .build())
}
