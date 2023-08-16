use must_future::MustBoxFuture;

/// The effect from a [`State::transition`].
///
/// Effects must be deterministically produced, meaning that for a given
/// `Action`, the same Effect output must be produced every time.
///
/// Note: The `Eq` bound, while not strictly necessary, is included here
/// for a few reasons:
/// - We want to discourage use of Futures as effects, because they
///     may produce nondeterministic results
/// - The `invariants` feature allows for automatic testing of state
///     machines for effect determinacy, which is only possible if the
///     effects have equality relationships
/// - In general, any type which does not have equality is probably a
///     poor choice for an Effect
pub trait Effect: Eq {}

impl<T> Effect for T where T: Eq {}
