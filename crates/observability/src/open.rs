use holochain_serialized_bytes::prelude::*;
use opentelemetry::api::{self, KeyValue, Link, SpanContext, TraceContextExt, Value};
use std::fmt::Write;
use tracing::{span::Attributes, Subscriber};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{registry::LookupSpan, Layer};

#[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes)]
pub struct WireContext {
    span_context: WireSpanContext,
    links: Option<WireLinks>,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, SerializedBytes, derive_more::From, derive_more::Into,
)]
pub struct WireLinks(pub Vec<WireLink>);

/// Needed because SB doesn't do u128
#[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes)]
pub struct WireLink {
    span_context: WireSpanContext,
    attributes: Vec<api::KeyValue>,
}

/// Needed because SB doesn't do u128
#[derive(Debug, Clone, Serialize, Deserialize, SerializedBytes)]
pub struct WireSpanContext {
    trace_id: String,
    span_id: api::SpanId,
    trace_flags: u8,
    is_remote: bool,
}

impl From<Link> for WireLink {
    fn from(l: Link) -> Self {
        WireLink {
            span_context: l.span_context().clone().into(),
            attributes: l.attributes().clone(),
        }
    }
}

impl From<WireLink> for Link {
    fn from(wl: WireLink) -> Self {
        Link::new(wl.span_context.into(), wl.attributes)
    }
}

impl From<SpanContext> for WireSpanContext {
    fn from(sc: SpanContext) -> Self {
        WireSpanContext {
            trace_id: sc.trace_id().to_u128().to_string(),
            span_id: sc.span_id(),
            trace_flags: sc.trace_flags(),
            is_remote: sc.is_remote(),
        }
    }
}

impl From<WireSpanContext> for SpanContext {
    fn from(wsc: WireSpanContext) -> Self {
        SpanContext::new(
            api::TraceId::from_u128(
                wsc.trace_id
                    .parse::<u128>()
                    .expect("Failed to parse trace id"),
            ),
            wsc.span_id,
            wsc.trace_flags,
            wsc.is_remote,
        )
    }
}

pub trait OpenSpanExt {
    fn get_context(&self) -> super::Context;
    fn get_context_bytes(&self) -> Vec<u8> {
        let wc: WireContext = (&self.get_context()).into();
        let sb: SerializedBytes = wc.try_into().expect("Failed to serialize tracing wire");
        let ub: UnsafeBytes = sb.into();
        ub.into()
    }
    fn set_context(&self, context: &super::Context);
    fn set_from_bytes(&self, bytes: Vec<u8>) {
        let sb: SerializedBytes = UnsafeBytes::from(bytes).into();
        let wire: WireContext = sb.try_into().expect("failed to deserialize tracing wire");
        let context: api::Context = wire.into();
        self.set_context(&context);
    }
    fn print_context(&self);
}

fn display_context(context: &super::Context) -> String {
    let mut out = String::new();
    write!(
        out,
        "trace_id: {}",
        context.span().span_context().trace_id().to_u128()
    )
    .ok();
    if let Some((_, links)) = context.get::<Vec<Link>>().and_then(|l| l.split_last()) {
        for link in links {
            write!(out, " ->").ok();
            for kv in link.attributes() {
                if let Value::String(v) = &kv.value {
                    write!(out, " {}: {};", kv.key.as_str(), v).ok();
                }
            }
        }
    }
    out
}

impl OpenSpanExt for tracing::Span {
    fn get_context(&self) -> api::Context {
        if self.is_disabled() {
            return api::Context::new();
        }
        let context = self.context();
        let span = context.span().span_context();
        let context = context.with_remote_span_context(span);
        get_followers(self, context)
    }

    fn set_context(&self, context: &api::Context) {
        if self.is_disabled() {
            return;
        }

        self.set_parent(context);
        set_followers(self, context);
    }

    fn print_context(&self) {
        if self.is_disabled() {
            return;
        }
        let context = self.get_context();
        self.in_scope(|| tracing::debug!(context = %display_context(&context)))
    }
}

fn get_followers(span: &tracing::Span, context: api::Context) -> api::Context {
    let mut links = None;
    span.with_subscriber(|(id, dispatch)| {
        if let Some(registry) = dispatch.downcast_ref::<tracing_subscriber::Registry>() {
            if let Some(span_ref) = registry.span(id) {
                let extensions = span_ref.extensions();
                if let Some(sb) = extensions.get::<api::SpanBuilder>() {
                    links = sb.links.clone();
                }
            }
        }
    });

    let links = links
        .map(|mut l| {
            if let Some(link) = create_link(span, &context) {
                l.push(link);
            }
            l
        })
        .or_else(|| create_link(span, &context).map(|l| vec![l]));

    match links {
        Some(links) => context.with_value(links),
        None => context,
    }
}

fn set_followers(span: &tracing::Span, context: &api::Context) {
    let new_links = context.get::<Vec<Link>>().cloned().unwrap_or_default();
    span.with_subscriber(|(id, dispatch)| {
        if let Some(registry) = dispatch.downcast_ref::<tracing_subscriber::Registry>() {
            if let Some(span_ref) = registry.span(id) {
                let mut extensions = span_ref.extensions_mut();
                if let Some(sb) = extensions.get_mut::<api::SpanBuilder>() {
                    if sb.links.is_none() {
                        sb.links = Some(Vec::new());
                    }
                    if let Some(links) = &mut sb.links {
                        links.extend(new_links);
                    }
                }
            }
        }
    });
}

fn create_link(span: &tracing::Span, context: &api::Context) -> Option<Link> {
    if let Some(meta) = span.metadata() {
        let mut kvs = Vec::with_capacity(2);
        kvs.push(KeyValue::new("span", meta.name()));
        if let (Some(file), Some(line)) = (meta.file(), meta.line()) {
            kvs.push(KeyValue::new("file", format!("{}:{}", file, line)));
        }
        let span_context = context.span().span_context();
        return Some(Link::new(span_context, kvs));
    }
    None
}

pub struct OpenLayer;

impl<S> Layer<S> for OpenLayer
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn new_span(
        &self,
        attrs: &Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("Span should not be missing");
        let mut extensions = span.extensions_mut();
        if let Some(parent) = attrs.parent() {
            let parent = ctx.span(parent).expect("Span should not be missing");
            let parent_extensions = parent.extensions();
            if let Some((p, s)) = parent_extensions
                .get::<api::SpanBuilder>()
                .and_then(|p| extensions.get_mut::<api::SpanBuilder>().map(|s| (p, s)))
            {
                s.links = p.links.clone()
            }
        } else if attrs.is_contextual() {
            if let Some(parent) = ctx.lookup_current() {
                let parent_extensions = parent.extensions();
                if let Some((p, s)) = parent_extensions
                    .get::<api::SpanBuilder>()
                    .and_then(|p| extensions.get_mut::<api::SpanBuilder>().map(|s| (p, s)))
                {
                    s.links = p.links.clone()
                }
            }
        }
    }
}

impl From<&api::Context> for WireContext {
    fn from(c: &api::Context) -> Self {
        let span_context = c.span().span_context().into();
        let links = c
            .get::<Vec<Link>>()
            .cloned()
            .map(|links| WireLinks(links.into_iter().map(WireLink::from).collect()));
        WireContext {
            span_context,
            links,
        }
    }
}

impl From<WireContext> for api::Context {
    fn from(wc: WireContext) -> Self {
        let mut c = api::Context::new().with_remote_span_context(wc.span_context.into());
        if let Some(links) = wc.links {
            let links: Vec<Link> = links.0.into_iter().map(Link::from).collect();
            c = c.with_value(links);
        }
        c
    }
}
