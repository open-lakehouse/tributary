use opentelemetry::{global, trace::TracerProvider};
use opentelemetry_otlp::SpanExporter;
use opentelemetry_sdk::{
    Resource,
    metrics::{MeterProviderBuilder, PeriodicReader, SdkMeterProvider},
    propagation::TraceContextPropagator,
    trace::SdkTracerProvider,
};
use tracing::level_filters::LevelFilter;
use tracing_opentelemetry::{MetricsLayer, OpenTelemetryLayer};
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

fn resource() -> Resource {
    Resource::builder().with_service_name("caspers").build()
}

pub(crate) fn init_meter_provider() -> SdkMeterProvider {
    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_temporality(opentelemetry_sdk::metrics::Temporality::default())
        .build()
        .unwrap();

    let reader = PeriodicReader::builder(exporter)
        .with_interval(std::time::Duration::from_secs(30))
        .build();

    // For debugging in development
    // let stdout_reader =
    //     PeriodicReader::builder(opentelemetry_stdout::MetricExporter::default()).build();

    let meter_provider = MeterProviderBuilder::default()
        .with_resource(resource())
        .with_reader(reader)
        // with_reader(stdout_reader)
        .build();

    global::set_meter_provider(meter_provider.clone());

    meter_provider
}

pub(crate) fn init_tracer_provider() {
    let exporter = SpanExporter::builder()
        .with_tonic()
        .build()
        .expect("Failed to create span exporter");
    let provider = SdkTracerProvider::builder()
        .with_resource(resource())
        .with_batch_exporter(exporter)
        .build();
    global::set_text_map_propagator(TraceContextPropagator::new());
    global::set_tracer_provider(provider);
}

/// Initialize tracing-subscriber and return OtelGuard for opentelemetry-related termination processing.
///
/// When `tui_mode` is true the fmt layer writes to `io::sink()` so that log
/// output does not corrupt the TUI alternate screen.
pub(crate) fn init_tracing_subscriber(tui_mode: bool) -> OtelGuard {
    let meter_provider = init_meter_provider();

    let exporter = SpanExporter::builder()
        .with_tonic()
        .build()
        .expect("Failed to create span exporter");
    let tracer_provider = SdkTracerProvider::builder()
        .with_resource(Resource::builder().with_service_name("caspers").build())
        .with_batch_exporter(exporter)
        .build();

    let tracer = tracer_provider.tracer("caspers");

    let env_filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .parse_lossy("caspers=debug,sim_core=debug,sim_kitchen=debug,sim_events=debug");

    // The global level filter prevents the exporter network stack
    // from reentering the globally installed OpenTelemetryLayer with
    // its own spans while exporting, as the libraries should not use
    // tracing levels below DEBUG.
    let otel_layer = OpenTelemetryLayer::new(tracer)
        .with_location(false)
        .with_threads(false);

    let metrics_layer = MetricsLayer::new(meter_provider.clone());

    let fmt_layer = if tui_mode {
        None
    } else {
        Some(tracing_subscriber::fmt::layer())
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(metrics_layer)
        .with(otel_layer)
        .init();

    OtelGuard {
        tracer_provider,
        meter_provider,
    }
}

pub(crate) struct OtelGuard {
    tracer_provider: SdkTracerProvider,
    meter_provider: SdkMeterProvider,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Err(err) = self.tracer_provider.shutdown() {
            eprintln!("{err:?}");
        }
        if let Err(err) = self.meter_provider.shutdown() {
            eprintln!("{err:?}");
        }
    }
}
