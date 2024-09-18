pub use off::*;

pub use context_wrap::MsgWrap;

mod context_wrap;

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

    /// Get the current span as message pack bytes.
    fn get_current_bytes() -> Vec<u8>;

    /// Set the context of this span.
    fn set_context(&self, context: Context);

    /// Set the context of the current span.
    fn set_current_context(context: Context);

    /// Set the current span context from message pack bytes.
    fn set_current_bytes(bytes: Vec<u8>);

    /// Display this spans context as a String.
    fn display_context(&self) -> String;
}
