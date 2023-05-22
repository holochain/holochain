use super::*;
#[derive(Debug, Clone, Default)]
pub struct Context;

#[derive(Debug, Clone)]
pub struct WireContext {
    span_context: WireSpanContext,
    links: Option<WireLinks>,
}

#[derive(Debug, Clone, derive_more::From, derive_more::Into)]
pub struct WireLinks(pub Vec<WireLink>);

#[derive(Debug, Clone)]
pub struct WireLink;

#[derive(Debug, Clone)]
pub struct WireSpanContext;

impl OpenSpanExt for tracing::Span {
    fn get_current_context() -> Context {
        Context
    }
    fn get_context(&self) -> Context {
        Context
    }

    fn get_current_bytes() -> Vec<u8> {
        Vec::with_capacity(0)
    }

    fn set_context(&self, _: Context) {}

    fn set_current_context(_: Context) {}
    fn set_current_bytes(_bytes: Vec<u8>) {}

    fn display_context(&self) -> String {
        String::with_capacity(0)
    }
}

impl std::fmt::Display for Context {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}
pub struct Config;
