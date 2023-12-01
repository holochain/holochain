#[cfg(not(feature = "opentelemetry-on"))]
pub use off::*;
#[cfg(feature = "opentelemetry-on")]
pub use on::*;

pub use context_wrap::MsgWrap;

#[allow(missing_docs)]
#[cfg(feature = "channels")]
pub mod channel;
mod context_wrap;

#[cfg(not(feature = "opentelemetry-on"))]
#[allow(missing_docs)]
mod off;

/// Opentelemetry span extension trait.
/// This trait provides helper methods to the
/// [tracing::Span] for crossing thread and
/// process boundaries.
pub trait OpenSpanExt {
    /// Get the context of this span.
    fn get_context(&self) -> Context;
    /// Get the context of the current span.
    fn get_current_context() -> Context;
    /// Get the context as message pack bytes for
    /// sending over process boundaries.
    fn get_context_bytes(&self) -> Vec<u8> {
        #[cfg(feature = "opentelemetry-on")]
        {
            use holochain_serialized_bytes::prelude::*;
            let wc: WireContext = (&self.get_context().0).into();
            // This shouldn't fail because there should always be a context
            // to serialize even if it's empty.
            let sb: SerializedBytes = wc.try_into().expect("Failed to serialize tracing wire");
            let ub: UnsafeBytes = sb.into();
            ub.into()
        }
        #[cfg(not(feature = "opentelemetry-on"))]
        {
            Vec::with_capacity(0)
        }
    }
    /// Get the current span as message pack bytes.
    fn get_current_bytes() -> Vec<u8>;
    /// Set the context of this span.
    fn set_context(&self, context: Context);
    /// Set the context of the current span.
    fn set_current_context(context: Context);
    #[allow(unused_variables)]
    /// Set the context of this span from bytes over the network.
    fn set_from_bytes(&self, bytes: Vec<u8>) {
        #[cfg(feature = "opentelemetry-on")]
        {
            use holochain_serialized_bytes::prelude::*;
            use opentelemetry::api;
            let sb: SerializedBytes = UnsafeBytes::from(bytes).into();
            let context = match WireContext::try_from(sb) {
                Ok(w) => api::Context::from(w).into(),
                Err(e) => {
                    tracing::error!(
                        msg = "Failed to deserialize tracing wire context into context"
                    );
                    Context::new()
                }
            };
            self.set_context(context);
        }
    }
    /// Set the current span context from message pack bytes.
    fn set_current_bytes(bytes: Vec<u8>);
    /// Display this spans context as a String.
    fn display_context(&self) -> String;
}

#[cfg(feature = "opentelemetry-on")]
#[warn(missing_docs)]
mod on {
    use once_cell::sync::OnceCell;

    use super::*;
    use holochain_serialized_bytes::prelude::*;
    use opentelemetry::api::{self, KeyValue, Link, SpanContext, TraceContextExt, Value};
    use std::sync::atomic::Ordering;
    use std::{collections::HashMap, ffi::OsString, sync::atomic::AtomicBool};
    use tracing::{span::Attributes, Subscriber};
    use tracing_opentelemetry::OpenTelemetrySpanExt;
    use tracing_subscriber::{registry::LookupSpan, Layer};

    pub(crate) static OPEN_ON: AtomicBool = AtomicBool::new(false);
    static CONFIG: OnceCell<Config> = OnceCell::new();
    static PROCESS_NAME: OnceCell<String> = OnceCell::new();

    /// The context holds the current state of a span.
    /// This can be used to transfer contexts across boundaries.
    /// A boundary crossing will show up as a follower for a span.
    #[derive(Debug, Clone, derive_more::From, derive_more::Into)]
    pub struct Context(pub(super) api::Context);

    /// Configuration for open telemetry tracing.
    /// These can all be configured by setting the
    /// `OPEN_TEL='process:true,file:false'`.
    /// They all have default settings.
    #[derive(Debug, Clone)]
    pub struct Config {
        /// Propagate the name of the process running
        /// on the sender side of the boundary crossing
        /// in the context and output when calling `spawn_context!()`.
        /// [Default: false]
        pub process: bool,
        /// Propagate the name of the file and line
        /// number of the sender side of the boundary crossing
        /// in the context and output when calling `spawn_context!()`.
        /// [Default: false]
        pub file: bool,
        /// Propagate the name of the span name
        /// of the sender side of the boundary crossing
        /// in the context and output when calling `spawn_context!()`.
        /// [Default: true]
        pub span_name: bool,
        /// Require there to be a span enabled for the sending side of
        /// the boundary crossing. [Default: true]
        pub require_span: bool,
    }

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

    impl Context {
        /// Create a new blank context.
        /// You usually won't do this and
        /// instead get the context from a span.
        pub fn new() -> Self {
            Context(api::Context::new())
        }
    }

    impl Default for Context {
        fn default() -> Self {
            Self::new()
        }
    }

    impl OpenSpanExt for tracing::Span {
        fn get_current_context() -> Context {
            let span = tracing::Span::current();
            span.get_context()
        }

        fn get_context(&self) -> Context {
            if should_not_run(self) {
                return Context::new();
            }
            let context = self.context();
            let span = context.span().span_context();
            let context = context.with_remote_span_context(span);
            get_followers(self, context).into()
        }

        fn get_current_bytes() -> Vec<u8> {
            let span = tracing::Span::current();
            span.get_context_bytes()
        }

        fn set_context(&self, context: Context) {
            if should_not_run(self) {
                return;
            }

            self.set_parent(&context.0);
            set_followers(self, &context.0);
        }

        fn set_current_context(context: Context) {
            let span = tracing::Span::current();
            span.set_context(context);
        }

        fn set_current_bytes(bytes: Vec<u8>) {
            let span = tracing::Span::current();
            span.set_from_bytes(bytes)
        }

        fn display_context(&self) -> String {
            if should_not_run(self) {
                return String::with_capacity(0);
            }
            let context = self.get_context();
            format!("{}", context)
        }
    }

    /// Emit a tracing event with the context of a span.
    /// ### Usage
    /// - `span_context!()` will emit a trace event with the current span.
    /// - `span_context!(current, Level::DEBUG)` will emit a debug event with the current span.
    /// - Pass a span in and emit a trace event:
    /// ```no_run
    /// # #[macro_use] extern crate holochain_trace;
    /// let span = tracing::debug_span!("my_pan");
    /// span_context!(span)
    /// ```
    /// - Pass a span in and emit a warn event:
    /// ```no_run
    /// # #[macro_use] extern crate holochain_trace;
    /// let span = tracing::debug_span!("my_pan");
    /// span_context!(span, tracing::Level::WARN)
    /// ```
    #[macro_export]
    macro_rules! span_context {
    (current, $lvl:expr) => {
        $crate::span_context!($crate::tracing::Span::current(), $lvl);
    };
    ($span:expr, $lvl:expr) => {{
        if $crate::tracing::level_enabled!($lvl) {
            if $crate::should_run(&$span) {
                let context = $crate::OpenSpanExt::get_context(&$span);
                $crate::tracing::event!(parent: &$span, $lvl, span_context = %context);
            }
        }

    }};
    ($span:expr) => {
        $crate::span_context!($span, $crate::tracing::Level::TRACE);
    };
    () => {
        $crate::span_context!($crate::tracing::Span::current(), $crate::tracing::Level::TRACE);
    };
}

    #[doc(hidden)]
    pub fn should_run(span: &tracing::Span) -> bool {
        !should_not_run(span)
    }

    fn should_not_run(span: &tracing::Span) -> bool {
        !OPEN_ON.load(Ordering::Relaxed) || (span.is_disabled() && Config::require_span())
    }

    impl std::fmt::Display for Context {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let context = &self.0;
            write!(
                f,
                "trace_id: {}",
                context.span().span_context().trace_id().to_u128()
            )?;
            if let Some((_, links)) = context.get::<Vec<Link>>().and_then(|l| l.split_last()) {
                for link in links {
                    write!(f, " ->")?;
                    for kv in link.attributes() {
                        if let Value::String(v) = &kv.value {
                            write!(f, " {}: {};", kv.key.as_str(), v)?;
                        }
                    }
                }
            }
            Ok(())
        }
    }

    pub(crate) fn init() {
        CONFIG.get_or_init(|| Config::from(std::env::var_os("OPEN_TEL")));
        PROCESS_NAME.get_or_init(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "not_found".to_string())
        });
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
        if !new_links.is_empty() {
            span.with_subscriber(|(id, dispatch)| {
                if let Some(registry) = dispatch.downcast_ref::<tracing_subscriber::Registry>() {
                    if let Some(span_ref) = registry.span(id) {
                        let mut extensions = span_ref.extensions_mut();
                        if let Some(sb) = extensions.get_mut::<api::SpanBuilder>() {
                            let mut new_links = new_links
                                .into_iter()
                                .rev()
                                .take_while(|link| {
                                    Some(link.span_context().span_id()) != sb.span_id
                                })
                                .collect::<Vec<_>>();
                            new_links.reverse();
                            sb.links = Some(new_links);
                        }
                    }
                }
            });
        }
    }

    fn create_link(span: &tracing::Span, context: &api::Context) -> Option<Link> {
        if let Some(meta) = span.metadata() {
            let mut kvs = Vec::with_capacity(2);
            if Config::span_name() {
                kvs.push(KeyValue::new("span", meta.name()));
            }
            if Config::file() {
                if let (Some(file), Some(line)) = (meta.file(), meta.line()) {
                    kvs.push(KeyValue::new("file", format!("{}:{}", file, line)));
                }
            }
            if Config::process() {
                kvs.push(KeyValue::new(
                    "process",
                    PROCESS_NAME
                        .get()
                        .cloned()
                        .unwrap_or_else(|| "not_found".to_string()),
                ))
            }
            let span_context = context.span().span_context();
            return Some(Link::new(span_context, kvs));
        }
        None
    }

    impl Config {
        fn require_span() -> bool {
            CONFIG
                .get()
                .map(|c| c.require_span)
                .unwrap_or_else(|| Config::default().require_span)
        }
        fn span_name() -> bool {
            CONFIG
                .get()
                .map(|c| c.span_name)
                .unwrap_or_else(|| Config::default().span_name)
        }
        fn file() -> bool {
            CONFIG
                .get()
                .map(|c| c.file)
                .unwrap_or_else(|| Config::default().file)
        }
        fn process() -> bool {
            CONFIG
                .get()
                .map(|c| c.process)
                .unwrap_or_else(|| Config::default().process)
        }
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

    impl Default for Config {
        fn default() -> Self {
            Self {
                process: false,
                file: false,
                span_name: true,
                require_span: true,
            }
        }
    }

    impl From<Option<OsString>> for Config {
        fn from(var: Option<OsString>) -> Self {
            let var = match var.and_then(|v| v.into_string().ok()) {
                Some(var) => var,
                None => return Self::default(),
            };
            let options = var.split(',').filter_map(|kv|{
                let kv = kv.split(':').map(|i|i.trim()).collect::<Vec<_>>();
                if kv.len() == 2 {
                    Some((kv[0], kv[1]))
                } else {
                    eprintln!("Failed to parse config from OPEN_TEL.\nFormat is `OPEN_TEL='key: value, key: value'`");
                    None
                }
            })
            .collect::<HashMap<&str, &str>>();
            let mut config = Config::default();
            if let Some(file) = options.get("file").and_then(|v| v.parse::<bool>().ok()) {
                config.file = file;
            }
            if let Some(process) = options.get("process").and_then(|v| v.parse::<bool>().ok()) {
                config.process = process;
            }
            if let Some(span_name) = options
                .get("span_name")
                .and_then(|v| v.parse::<bool>().ok())
            {
                config.span_name = span_name;
            }
            if let Some(require_span) = options
                .get("require_span")
                .and_then(|v| v.parse::<bool>().ok())
            {
                config.require_span = require_span;
            }

            config
        }
    }
}
