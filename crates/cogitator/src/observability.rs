//! Tracing subscriber layers (specs/rust-port/05 §6.3): `--debug` file layer,
//! opt-in OTLP export, tokio runtime tracing / optional tokio-console.
//! Nothing here ever writes to stdout — that belongs to LSP frames.

use anyhow::Result;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Keeps the OTLP provider alive; flushed on drop / `exit` (01 §3.3).
pub struct Guard {
    otlp: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
    metrics: Option<opentelemetry_sdk::metrics::SdkMeterProvider>,
}

impl Guard {
    pub fn shutdown(&self) {
        if let Some(provider) = &self.otlp {
            let _ = provider.shutdown();
        }
        if let Some(provider) = &self.metrics {
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
    // PER-LAYER filters: a single global filter at `info` used to hide
    // every debug-level span from OTLP; each layer now filters itself.
    let layer_filter =
        || EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    // `--debug` file layer: log.txt beside the executable (01 §5). An
    // unwritable install dir (e.g. the Nix store) degrades to a stderr
    // warning instead of failing startup.
    let file_layer = if debug {
        let dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_default();
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("log.txt"))
        {
            Ok(file) => Some(
                tracing_subscriber::fmt::layer()
                    .with_writer(file)
                    .with_ansi(false)
                    .with_target(true),
            ),
            Err(err) => {
                eprintln!(
                    "cogitator: --debug log.txt not writable in {} ({err});                      continuing without the file log",
                    dir.display()
                );
                None
            }
        }
    } else {
        None
    };
    let file_layer = file_layer.map(|layer| {
        use tracing_subscriber::Layer as _;
        layer.with_filter(layer_filter())
    });

    // Opt-in OTLP layer (05 §6.3): --otlp[=endpoint] or the standard env var.
    let enabled_by_env = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok();
    let otlp_provider = match (otlp, enabled_by_env) {
        (None, false) => None,
        (endpoint, _) => Some(build_otlp(endpoint.filter(|e| !e.is_empty()))?),
    };
    // metrics ride the same gate and resource as traces (05 §6.3)
    let meter_provider = match &otlp_provider {
        Some(_) => {
            let provider = build_otlp_metrics(otlp.filter(|e| !e.is_empty()))?;
            crate::metrics::init(&provider);
            Some(provider)
        }
        None => None,
    };
    let otlp_layer = otlp_provider.as_ref().map(|provider| {
        use opentelemetry::trace::TracerProvider as _;
        use tracing_subscriber::Layer as _;
        tracing_opentelemetry::layer()
            .with_tracer(provider.tracer("dbt-language-server"))
            .with_filter(layer_filter())
    });

    #[cfg(feature = "tokio-console")]
    let console_layer = Some(console_subscriber::spawn());
    #[cfg(not(feature = "tokio-console"))]
    let console_layer: Option<tracing_subscriber::layer::Identity> = None;

    tracing_subscriber::registry()
        .with(file_layer)
        .with(otlp_layer)
        .with(console_layer)
        .init();

    let _ = GUARD.set(Guard {
        otlp: otlp_provider,
        metrics: meter_provider,
    });
    Ok(())
}

fn build_otlp_metrics(
    endpoint: Option<&str>,
) -> Result<opentelemetry_sdk::metrics::SdkMeterProvider> {
    use opentelemetry_otlp::WithExportConfig;

    let mut exporter = opentelemetry_otlp::MetricExporter::builder().with_tonic();
    if let Some(endpoint) = endpoint {
        exporter = exporter.with_endpoint(endpoint.to_string());
    }
    let exporter = exporter.build()?;
    Ok(opentelemetry_sdk::metrics::SdkMeterProvider::builder()
        .with_periodic_exporter(exporter)
        .with_resource(otlp_resource())
        .build())
}

fn otlp_resource() -> opentelemetry_sdk::Resource {
    use opentelemetry::KeyValue;
    opentelemetry_sdk::Resource::builder()
        .with_service_name("dbt-language-server") // 05 §6.3's resource name
        .with_attributes([KeyValue::new("service.version", crate::VERSION)])
        .build()
}

fn build_otlp(endpoint: Option<&str>) -> Result<opentelemetry_sdk::trace::SdkTracerProvider> {
    use opentelemetry_otlp::WithExportConfig;

    let mut exporter = opentelemetry_otlp::SpanExporter::builder().with_tonic();
    if let Some(endpoint) = endpoint {
        exporter = exporter.with_endpoint(endpoint.to_string());
    }
    let exporter = exporter.build()?;

    Ok(opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(otlp_resource())
        .build())
}
