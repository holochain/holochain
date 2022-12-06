use std::hash::Hash;

use kitsune_p2p_types::KOpHash;

/// The granularity once we're > i16::MAX
const GRAN: usize = 4096;

// const G: f64 = 1690.0; // x / (ln_phi 128000000)
// const LOW: f64 = 15.0 * G;
// const THRESH: u16 = 30000;

/// Roughly track an approximate integer value.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct RoughInt(i16);

impl std::fmt::Debug for RoughInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

impl RoughInt {
    /// Get the full value from the rough int
    pub fn get(&self) -> usize {
        if self.0 > 0 {
            // positive is exact value
            self.0 as usize
        } else {
            // negative is in chunks of size GRAN
            (-self.0) as usize * GRAN
        }
    }

    /// Set the rough int from the full value
    pub fn set(&mut self, v: usize) -> Self {
        if v <= i16::MAX as usize {
            // if we're under i16::MAX, we can store the exact value
            self.0 = v as i16
        } else {
            // otherwise, divide by GRAN and store as negative
            self.0 = -(std::cmp::min(i16::MAX as usize, v / GRAN) as i16);
        }
        *self
    }
}

impl From<usize> for RoughInt {
    fn from(v: usize) -> Self {
        Self::default().set(v)
    }
}

/// An op hash combined with an approximate size of the op
pub type OpHashSized = RoughSized<KOpHash>;

/// Some data which has a RoughInt assigned for its size
#[derive(
    Clone,
    Debug,
    serde::Deserialize,
    serde::Serialize,
    derive_more::From,
    derive_more::Into,
    derive_more::Constructor,
    derive_more::Deref,
)]
pub struct RoughSized<T> {
    /// The data to be sized
    #[deref]
    data: T,
    /// The approximate size of the hash.
    // TODO: remove the option, which will require adding sizes for Recent gossip as well
    size: Option<RoughInt>,
}

impl<T> RoughSized<T> {
    /// Break into constituent parts
    pub fn into_inner(self) -> (T, Option<RoughInt>) {
        (self.data, self.size)
    }

    /// Accessor
    pub fn data_ref(&self) -> &T {
        &self.data
    }

    /// Accessor
    pub fn maybe_size(&self) -> Option<RoughInt> {
        self.size
    }

    /// Accessor
    pub fn size(&self) -> RoughInt {
        self.size.unwrap_or_default()
    }
}

impl<T: Clone> RoughSized<T> {
    /// Accessor
    pub fn data(&self) -> T {
        self.data.clone()
    }
}

impl<T: Hash> Hash for RoughSized<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // size is omitted from the hash
        self.data.hash(state);
    }
}

impl<T: PartialEq> PartialEq for RoughSized<T> {
    fn eq(&self, other: &Self) -> bool {
        // size is omitted from the equality
        self.data == other.data
    }
}

impl<T: Eq> Eq for RoughSized<T> {}
