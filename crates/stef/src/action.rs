use must_future::MustBoxFuture;

use crate::util::box_fut;

/// The action consumed by a [`State::transition`]
pub trait Action: Sized {}
impl<T> Action for T where T: Sized {}

/// A type represention a state transition for some [`State`].
///
/// Every [`Transition`] specifies a "compact" representation with a smaller size,
/// which is useful when collecting a list of transitions for analysis or playback,
/// so as not to let the size of the stored Transactions get overwhelming.
/// If a Transition is already as compact as it can be, you can use [`TransitionCompact`]
/// to use Self as the [`Compact`] type.
///
/// The [`Expander`] is the source of the extra information needed rebuild the full
/// Transition from the Compact type.
///
/// For instance, if a transition involves adding a large chunk of data to a State,
/// in the Compact representation you may choose to replace that chunk of data with a
/// small ID or hash instead, so that in case you store the Transitions, you're not
/// duplicating that large amount of data. When "expanding" back to the full transition
/// from the compact representation, you can use the State itself as the Expander
/// to look up the original item to insert into the reconstituted Transition.
pub trait ActionReplay: Action + Send + Sync {
    /// The compact representation for this Transition
    type Compact: serde::Serialize + for<'a> serde::Deserialize<'a>;

    /// The extra context needed to reconstitute a full Transition from the Compact.
    type Expander;

    /// Go from full to compact
    fn compact(&self) -> Self::Compact;

    /// Go from compact to full, using the Expander's extra context
    fn expand(ctx: &Self::Expander, compact: Self::Compact) -> MustBoxFuture<anyhow::Result<Self>>;
}

/// Defines a Transition whose full and Compact representations are the same.
pub trait ActionCompact:
    ActionReplay<Compact = Self> + Clone + serde::Serialize + for<'a> serde::Deserialize<'a>
{
}
impl<T> ActionReplay for T
where
    T: ActionCompact + 'static,
{
    type Compact = Self;
    type Expander = ();

    fn compact(&self) -> Self::Compact {
        self.clone()
    }

    fn expand(_: &Self::Expander, compact: Self::Compact) -> MustBoxFuture<anyhow::Result<Self>> {
        box_fut(Ok(compact))
    }
}

impl ActionCompact for () {}
impl ActionCompact for u8 {}
impl ActionCompact for u16 {}
impl ActionCompact for u32 {}
impl ActionCompact for u64 {}
impl ActionCompact for u128 {}
impl ActionCompact for i8 {}
impl ActionCompact for i16 {}
impl ActionCompact for i32 {}
impl ActionCompact for i128 {}
impl ActionCompact for i64 {}
impl ActionCompact for f32 {}
impl ActionCompact for f64 {}
impl ActionCompact for String {}
impl<T: 'static> ActionCompact for Box<T> where T: ActionCompact {}
impl<T: 'static> ActionCompact for Option<T> where T: ActionCompact {}
impl<T: 'static> ActionCompact for Vec<T> where T: ActionCompact {}
// impl<'a> ActionCompact for &'a str {}
// impl<'a, T> ActionCompact for &'a [T] {}
