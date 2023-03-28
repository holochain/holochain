use super::*;

/// Wrap a channel message in a span context.
/// The context is automatically propagated to
/// the current span by calling `msg_wrap.inner()`.
/// The context is automatically propagated from
/// the current span by calling `t.into()`.
/// If you wish to avoid either of these propagations
/// you can use `msg_wrap.without_context()` and
/// `MsgWrap::from_no_context(t)` respectively.
pub struct MsgWrap<T> {
    t: T,
    context: Option<Context>,
}

impl<T> MsgWrap<T> {
    /// Create a T wrapped in a Context.
    /// If you just need the current context use
    /// `t.into()`.
    pub fn new(t: T, context: Context) -> Self {
        Self {
            t,
            context: Some(context),
        }
    }
    /// Get the inner type and propagate the context to
    /// the current span.
    pub fn inner(self) -> T {
        if let Some(context) = self.context {
            tracing::Span::set_current_context(context);
        }
        self.t
    }
    /// Get the inner type without propagating the context.
    pub fn without_context(self) -> T {
        self.t
    }

    /// Create a wrapped T with no Context.
    pub fn from_no_context(t: T) -> Self {
        Self { t, context: None }
    }

    /// Unwrap the wrapped T into a T and Context.
    /// If you just need to propagate the context to
    /// the current span use `msg_wrap.inner()`
    pub fn into_parts(self) -> (T, Context) {
        (self.t, self.context.unwrap_or_default())
    }
}

impl<T> From<T> for MsgWrap<T> {
    /// Create a wrapped T with the context from
    /// the current span.
    fn from(t: T) -> Self {
        let span = tracing::Span::current();
        let context = if span.is_disabled() {
            None
        } else {
            Some(span.get_context())
        };

        Self { t, context }
    }
}

impl<T> std::fmt::Debug for MsgWrap<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self.t))
    }
}
