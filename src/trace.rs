pub use opentelemetry::trace::{
    get_active_span, mark_span_as_active, FutureExt, Span as _, SpanBuilder, SpanContext, SpanId,
    TraceContextExt, TraceFlags, TraceId, TraceState, Tracer as OtelTracer, TracerProvider as _,
    WithContext,
};
pub use opentelemetry::Context;
pub use opentelemetry_sdk::trace::IdGenerator;
pub use opentelemetry_sdk::trace::RandomIdGenerator;
pub use opentelemetry_sdk::{
    trace::BatchConfig as BatchTraceConfig, trace::Config as TracerProviderConfig,
    trace::Span as TraceSpan, trace::Tracer,
};

use opentelemetry::global;
use opentelemetry_sdk::runtime::Tokio;
use opentelemetry_sdk::{trace::BatchSpanProcessor, trace::TracerProvider};
use opentelemetry_stdout::SpanExporter;
use std::fmt::Debug;
use std::ops::Deref;
use std::sync::{Arc, OnceLock};
use sulid::SulidGenerator;

/// Re-export opentelemetry::trace;
pub mod otel_trace {
    pub use opentelemetry::trace::*;
}

// const INSTRUMENTATION_LIBRARY_NAME: &str = "opentelemetry-appender-tracing";

/// The global `Tracer` singleton.
static GLOBAL_TRACER: OnceLock<Tracer> = OnceLock::new();

/// Returns the global &'static Tracer
pub fn tracer() -> &'static Tracer {
    GLOBAL_TRACER.get().unwrap()
}

/// Returns the global Arc<Tracer>
#[inline]
pub fn arc_tracer() -> ArcTracer {
    tracer().into()
}

pub(crate) fn init_trace(
    service_name: String,
    service_version: String,
    use_stdout_exporter: bool,
    batch_trace_config: Option<BatchTraceConfig>,
    tracer_provider_config: TracerProviderConfig,
) -> anyhow::Result<Tracer> {
    let mut tracer_provider = TracerProvider::builder();
    if use_stdout_exporter {
        let span_exporter = SpanExporter::default();
        if let Some(batch_trace_config) = batch_trace_config {
            let batch = BatchSpanProcessor::builder(span_exporter, Tokio)
                .with_batch_config(batch_trace_config)
                .build();
            tracer_provider = tracer_provider.with_span_processor(batch);
        } else {
            tracer_provider = tracer_provider.with_simple_exporter(span_exporter);
        }
    } else {
        let span_exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .build_span_exporter()?;
        if let Some(batch_trace_config) = batch_trace_config {
            let batch = BatchSpanProcessor::builder(span_exporter, Tokio)
                .with_batch_config(batch_trace_config)
                .build();
            tracer_provider = tracer_provider.with_span_processor(batch);
        } else {
            tracer_provider = tracer_provider.with_simple_exporter(span_exporter);
        }
    }

    let tracer_provider: TracerProvider =
        tracer_provider.with_config(tracer_provider_config).build();

    let tracer = tracer_provider
        .tracer_builder(service_name)
        .with_version(service_version)
        .build();

    global::set_tracer_provider(tracer_provider);

    GLOBAL_TRACER.set(tracer.clone()).unwrap();

    Ok(tracer)
}

/// Create trace span customarily.
pub fn tracer_span(builder: SpanBuilder, parent_cx: Option<&Context>) -> TraceSpan {
    let tracer = tracer();
    if let Some(parent_cx) = parent_cx {
        tracer.build_with_context(builder, parent_cx)
    } else {
        tracer.build(builder)
    }
}

/// Extension trait allowing futures, streams, and sinks to be traced with a span.
pub trait FutureTraceExt: FutureExt {
    /// Pass the span of opentelemetry to the current context of tracing.
    fn with_current_context_span(self, otel_span: TraceSpan) -> WithContext<Self> {
        let otel_cx = Context::current_with_span(otel_span);
        self.with_context(otel_cx)
    }
}

impl<T: FutureExt> FutureTraceExt for T {}

/// Generate trace_id using the Snowflake-inspired ULIDs (SULIDs),
/// and generate span_id using a random number generator.
pub struct MyIdGenerator {
    trace_id: SulidGenerator,
    span_id: RandomIdGenerator,
}

impl IdGenerator for MyIdGenerator {
    fn new_trace_id(&self) -> TraceId {
        TraceId::from(self.trace_id.generate().u128())
    }

    fn new_span_id(&self) -> SpanId {
        self.span_id.new_span_id()
    }
}

impl Debug for MyIdGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MyIdGenerator")
            .field("trace_id", &"<sulid::SulidGenerator>")
            .field("span_id", &self.span_id)
            .finish()
    }
}

/// ArcTracer implement: Tracer + Sync + Send + 'static
pub struct ArcTracer(Arc<&'static Tracer>);

impl From<&'static Tracer> for ArcTracer {
    fn from(value: &'static Tracer) -> Self {
        Self(Arc::new(value))
    }
}

impl From<Arc<&'static Tracer>> for ArcTracer {
    fn from(value: Arc<&'static Tracer>) -> Self {
        Self(value)
    }
}

impl Deref for ArcTracer {
    type Target = Tracer;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl OtelTracer for ArcTracer {
    type Span = <Tracer as OtelTracer>::Span;

    fn build_with_context(&self, builder: SpanBuilder, parent_cx: &Context) -> Self::Span {
        self.0.build_with_context(builder, parent_cx)
    }
}
