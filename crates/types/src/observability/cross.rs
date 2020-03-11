use tracing::{span, subscriber::Subscriber, Span};
use tracing_core::{field::Visit, Field};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer, Registry};
/// A layer to allow cross process tracing using unique id's
pub struct CrossLayer {}

impl CrossLayer {
    pub fn new() -> Self {
        CrossLayer {}
    }
}

#[derive(Debug, Clone)]
struct TraceId(String);
struct ContextVisitor {
    trace_id: Option<TraceId>,
}

impl ContextVisitor {
    fn new() -> Self {
        ContextVisitor { trace_id: None }
    }
}

impl Visit for ContextVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "trace_id" {
            self.trace_id = Some(TraceId(format!("{:?}", value)));
        }
    }
}
impl<S> Layer<S> for CrossLayer
where
    S: Subscriber,
    S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        if let Some(_) = attrs.metadata().fields().field("trace_id") {
            let mut visitor = ContextVisitor::new();
            attrs.record(&mut visitor);
            if let Some(trace_id) = visitor.trace_id {
                let span = ctx
                    .span(id)
                    .expect("Should always be able to find self span");
                span.extensions_mut().replace(trace_id);
            }
        } else {
            if let Some(trace_id) = check_parents(attrs, &ctx) {
                let span = ctx
                    .span(id)
                    .expect("Should always be able to find self span");
                span.extensions_mut().replace(trace_id);
            }
        }
    }
}
fn check_parents<S>(attrs: &span::Attributes, ctx: &Context<'_, S>) -> Option<TraceId>
where
    S: Subscriber,
    S: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    let current = ctx.current_span();
    attrs
        .parent()
        .or_else(|| current.id())
        .and_then(|parent| ctx.span(parent))
        .and_then(|span| span.extensions().get::<TraceId>().cloned())
}

pub(crate) fn get_trace_id(span: Span) -> Option<String> {
    span.id().and_then(|id| {
        tracing::dispatcher::get_default(|dispatch| {
            dispatch
                .downcast_ref::<Registry>()
                .and_then(|registry| {
                    registry.span(&id).and_then(|span_ref| {
                        span_ref.extensions().get::<TraceId>().map(|t| t.0.clone())
                    })
                })
        })
    })
}
