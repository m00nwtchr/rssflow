use std::str::FromStr;

use opentelemetry::{KeyValue, global, propagation::Injector};
use opentelemetry_http::{HeaderExtractor, HeaderInjector};
use opentelemetry_otlp::{MetricExporter, SpanExporter};
use opentelemetry_sdk::{
	Resource,
	metrics::{MeterProviderBuilder, PeriodicReader, SdkMeterProvider},
	propagation::TraceContextPropagator,
	trace::SdkTracerProvider,
};
use opentelemetry_semantic_conventions::{
	SCHEMA_URL,
	attribute::{SERVICE_NAME, SERVICE_VERSION},
};
use tonic::metadata::{KeyRef, MetadataKey, MetadataMap};
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::ServiceInfo;

#[must_use]
pub fn init_meter_provider(info: &ServiceInfo) -> SdkMeterProvider {
	let exporter = MetricExporter::builder()
		.with_tonic()
		.build()
		.expect("Failed to create metric exporter");

	let reader = PeriodicReader::builder(exporter)
		.with_interval(std::time::Duration::from_secs(30))
		.build();

	let meter_provider = MeterProviderBuilder::default()
		.with_resource(info.into())
		.with_reader(reader)
		// .with_reader(stdout_reader)
		.build();

	global::set_meter_provider(meter_provider.clone());
	meter_provider
}

#[must_use]
pub fn init_tracer_provider(info: &ServiceInfo) -> SdkTracerProvider {
	let exporter = SpanExporter::builder()
		.with_tonic()
		.build()
		.expect("Failed to create span exporter");

	let tracer_provider = SdkTracerProvider::builder()
		.with_resource(info.into())
		.with_batch_exporter(exporter)
		.build();

	global::set_text_map_propagator(TraceContextPropagator::new());
	global::set_tracer_provider(tracer_provider.clone());

	tracer_provider
}

impl From<&ServiceInfo> for Resource {
	fn from(value: &ServiceInfo) -> Self {
		Resource::builder()
			.with_service_name(value.pkg)
			.with_schema_url(
				[
					KeyValue::new(SERVICE_NAME, value.pkg),
					KeyValue::new(SERVICE_VERSION, value.version),
					// KeyValue::new(DEPLOYMENT_ENVIRONMENT_NAME, "develop"),
				],
				SCHEMA_URL,
			)
			.build()
	}
}

pub fn accept_trace<T>(request: &tonic::Request<T>) {
	let parent_context =
		global::get_text_map_propagator(|p| p.extract(&MetadataExtractor(request.metadata())));
	Span::current().set_parent(parent_context);
}

/// Trace context propagation: send the trace context by injecting it into the metadata of the given
/// request.
pub fn send_trace<T>(request: &mut tonic::Request<T>) {
	let context = Span::current().context();
	global::get_text_map_propagator(|p| {
		p.inject_context(&context, &mut MetadataInjector(request.metadata_mut()));
	});
}

struct MetadataInjector<'a>(&'a mut MetadataMap);

struct MetadataExtractor<'a>(&'a MetadataMap);

impl<'a> opentelemetry::propagation::Extractor for MetadataExtractor<'a> {
	fn get(&self, key: &str) -> Option<&str> {
		self.0.get(key).and_then(|metadata| metadata.to_str().ok())
	}

	fn keys(&self) -> Vec<&str> {
		self.0
			.keys()
			.map(|key| match key {
				KeyRef::Ascii(v) => v.as_str(),
				KeyRef::Binary(v) => v.as_str(),
			})
			.collect::<Vec<_>>()
	}
}

impl Injector for MetadataInjector<'_> {
	fn set(&mut self, key: &str, value: String) {
		if let Ok(key) = MetadataKey::from_str(key) {
			if let Ok(val) = value.parse() {
				self.0.insert(key, val);
			}
		}
	}
}
