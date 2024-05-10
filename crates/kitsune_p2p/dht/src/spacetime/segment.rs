use super::*;

/// An Offset represents the position of the left edge of some Segment.
/// Offsets must be paired with a *power* to map to quantum coordinates.
/// The absolute DhtLocation of the offset is determined by the "power" of its
/// context, and topology of the space, by:
///
///   dht_location = offset * 2^pow * quantum_size
pub trait Offset: Sized + Copy + Clone + Deref<Target = u32> + From<u32> {
    /// The type of quantum to map to, which also implies the absolute coordinates
    type Quantum: Quantum;

    /// Get the absolute coordinate for this Offset
    fn to_absolute(&self, dim: QDim<Self>, power: u8) -> QAbs<Self>;

    /// Get the quantum coordinate for this Offset
    fn to_quantum(&self, power: u8) -> Self::Quantum;

    /// Get the nearest rounded-down Offset for the given Loc
    fn from_absolute_rounded(loc: Loc, dim: QDim<Self>, power: u8) -> Self;
}

pub(crate) type QAbs<O> = <<O as Offset>::Quantum as Quantum>::Absolute;
pub(crate) type QDim<O> = <<O as Offset>::Quantum as Quantum>::Dim;

/// An Offset in space.
#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    derive_more::Add,
    derive_more::Sub,
    derive_more::Mul,
    derive_more::Div,
    derive_more::Deref,
    derive_more::DerefMut,
    derive_more::From,
    derive_more::Into,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
#[serde(transparent)]
pub struct SpaceOffset(pub u32);

/// An Offset in time.
#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    derive_more::Add,
    derive_more::Sub,
    derive_more::Mul,
    derive_more::Div,
    derive_more::Deref,
    derive_more::DerefMut,
    derive_more::From,
    derive_more::Into,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
#[serde(transparent)]
pub struct TimeOffset(pub u32);

impl Offset for SpaceOffset {
    type Quantum = SpaceQuantum;

    /// Get the absolute coordinate for this Offset
    fn to_absolute(&self, dim: SpaceDimension, power: u8) -> Loc {
        self.wrapping_mul(dim.quantum)
            .wrapping_mul(pow2(power))
            .into()
    }

    /// Get the quantum coordinate for this Offset
    fn to_quantum(&self, power: u8) -> Self::Quantum {
        self.wrapping_mul(pow2(power)).into()
    }

    /// Get the nearest rounded-down Offset for the given Loc
    fn from_absolute_rounded(loc: Loc, dim: SpaceDimension, power: u8) -> Self {
        (loc.as_u32() / dim.quantum / pow2(power)).into()
    }
}

impl Offset for TimeOffset {
    type Quantum = TimeQuantum;

    /// Get the absolute coordinate for this Offset
    fn to_absolute(&self, dim: TimeDimension, power: u8) -> Timestamp {
        Timestamp::from_micros(self.wrapping_mul(dim.quantum).wrapping_mul(pow2(power)) as i64)
    }

    /// Get the quantum coordinate for this Offset
    fn to_quantum(&self, power: u8) -> Self::Quantum {
        self.wrapping_mul(pow2(power)).into()
    }

    /// Get the nearest rounded-down Offset for the given Loc
    fn from_absolute_rounded(loc: Loc, dim: TimeDimension, power: u8) -> Self {
        (loc.as_u32() / dim.quantum / pow2(power)).into()
    }
}

/// Any interval in space or time is represented by a node in a tree, so our
/// way of describing intervals uses tree coordinates as well:
/// The length of an interval is 2^(power), and the position of its left edge
/// is at (offset * length).
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub struct Segment<O: Offset> {
    /// The exponent, where length = 2^power
    pub power: u8,
    /// The offset from the origin, measured in number of lengths
    pub offset: O,
}

impl<O: Offset> Segment<O> {
    /// Constructor
    pub fn new<OO: Into<O>>(power: u8, offset: OO) -> Self {
        Self {
            power,
            offset: offset.into(),
        }
    }

    /// How many quanta does this segment cover?
    pub fn num_quanta(&self) -> u64 {
        // If power is 32, this overflows a u32
        2u64.pow(self.power.into())
    }

    /// The length, in absolute terms (Location or microseconds of time)
    pub fn absolute_length(&self, dim: QDim<O>) -> u64 {
        let q = dim.into().quantum as u64;
        // If power is 32, this overflows a u32
        self.num_quanta() * q
    }

    /// Get the quanta which bound this segment
    pub fn quantum_bounds(&self, dim: QDim<O>) -> (O::Quantum, O::Quantum) {
        let n = self.num_quanta();
        let a = (n * u64::from(*self.offset)) as u32;
        (
            O::Quantum::from(a).normalized(dim),
            O::Quantum::from(a.wrapping_add(n as u32).wrapping_sub(1)).normalized(dim),
        )
    }

    /// The segment contains the given quantum coord
    pub fn contains_quantum(&self, dim: QDim<O>, coord: O::Quantum) -> bool {
        let (lo, hi) = self.quantum_bounds(dim);
        let coord = coord.normalized(dim);
        if lo <= hi {
            lo <= coord && coord <= hi
        } else {
            lo <= coord || coord <= hi
        }
    }

    /// Split a segment in half
    pub fn bisect(&self) -> Option<[Self; 2]> {
        if self.power == 0 {
            // Can't split a quantum value (a leaf has no children)
            None
        } else {
            let power = self.power - 1;
            Some([
                Segment::new(power, O::from(self.offset.wrapping_mul(2))),
                Segment::new(power, O::from(self.offset.wrapping_mul(2).wrapping_add(1))),
            ])
        }
    }
}

impl SpaceSegment {
    /// Get the start and end bounds, in absolute Loc coordinates, for this segment
    pub fn loc_bounds(&self, dim: impl SpaceDim) -> (Loc, Loc) {
        let (a, b): (u32, u32) = space_bounds(dim.into().into(), self.power, self.offset, 1);
        (Loc::from(a), Loc::from(b))
    }
}

impl TimeSegment {
    /// Get the start and end bounds, in absolute Timestamp coordinates, for this segment
    pub fn timestamp_bounds(&self, topo: &Topology) -> (Timestamp, Timestamp) {
        let (a, b): (i64, i64) = time_bounds64(topo.time.into(), self.power, self.offset, 1);
        let o = topo.time_origin.as_micros();
        (Timestamp::from_micros(a + o), Timestamp::from_micros(b + o))
    }
}

/// Alias
pub type SpaceSegment = Segment<SpaceOffset>;

/// Alias
pub type TimeSegment = Segment<TimeOffset>;

/// Convert to a literal location in space using the given [Dimension] and optionally a power value
/// which can further quantize the space. The power can be set to 0 to use the [Dimension]'s quantum
/// as is.
///
/// The quantized input location is expressed as an offset and a count of quanta. The quantum is
/// computed first, then the locations are then computed as:
/// (`offset * quantum`, `offset * quantum + count * quantum`).
///
/// When the `power` is used, it should be a power of two, between 1 and 31. The locations are then
/// computed as:
/// (`offset * quantum * 2^power`, `offset * quantum * 2^power + count * quantum * 2^power`).
pub(super) fn space_bounds<N: From<u32>>(
    dim: Dimension,
    power: u8,
    offset: SpaceOffset,
    count: u32,
) -> (N, N) {
    debug_assert_ne!(dim.quantum, 0);
    debug_assert_ne!(count, 0);
    let q = dim.quantum.wrapping_mul(pow2(power));
    let start = offset.wrapping_mul(q);
    let len = count.wrapping_mul(q);
    (start.into(), start.wrapping_add(len).wrapping_sub(1).into())
}

// TODO is wrapping meaningful for time? Should we saturate instead?
pub(super) fn time_bounds64<N: From<i64>>(
    dim: Dimension,
    power: u8,
    offset: TimeOffset,
    count: u32,
) -> (N, N) {
    debug_assert_ne!(dim.quantum, 0);
    debug_assert_ne!(count, 0);
    let q = dim.quantum as i64 * 2i64.pow(power.into());
    let start = (*offset as i64).wrapping_mul(q);
    let len = (count as i64).wrapping_mul(q);
    (start.into(), start.wrapping_add(len).wrapping_sub(1).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_space_bounds() {
        let dim = SpaceDimension::standard();
        let (lower, upper) = space_bounds::<u32>(dim.into(), 0, 5.into(), 2);

        assert_eq!(lower, dim.quantum * 5);
        assert_eq!(upper, dim.quantum * 5 + dim.quantum * 2 - 1);
    }

    #[test]
    fn simple_wrapping_space_bounds() {
        let dim = SpaceDimension::standard();
        let (lower, upper) = space_bounds::<u32>(dim.into(), 0, 5.into(), pow2(32 - dim.quantum_power) - 5 + 1);

        assert!(upper < lower, "lower: {lower}, upper: {upper}");

        assert_eq!(lower, dim.quantum * 5);
        assert_eq!(upper, dim.quantum - 1);
    }

    #[test]
    fn space_bounds_with_quantization() {
        let dim = SpaceDimension::standard();

        // This power further scales the quantum size.
        // This is used to apply Arq quantization on top of the space's own quantization.
        let (lower, upper) = space_bounds::<u32>(dim.into(), 2, 5.into(), 2);

        let scaled_quantum = dim.quantum * pow2(2);
        assert_eq!(lower, scaled_quantum * 5);
        assert_eq!(upper, scaled_quantum * 5 + scaled_quantum * 2 - 1);
    }

    #[test]
    fn simple_time_bounds() {
        let dim = TimeDimension::standard();
        let (lower, upper) = time_bounds64::<i64>(dim.into(), 0, 5.into(), 2);

        assert_eq!(lower, dim.quantum as i64 * 5);
        assert_eq!(upper, dim.quantum as i64 * 5 + dim.quantum as i64  * 2 - 1);
    }

    #[test]
    fn time_bounds_with_quantization() {
        let dim = TimeDimension::standard();

        // This power further scales the quantum size.
        // This is used to apply Arq quantization on top of the time's own quantization.
        let (lower, upper) = time_bounds64::<i64>(dim.into(), 2, 5.into(), 2);

        let scaled_quantum = (dim.quantum * pow2(2)) as i64;
        assert_eq!(lower, scaled_quantum * 5);
        assert_eq!(upper, scaled_quantum * 5 + scaled_quantum * 2 - 1);
    }
}
