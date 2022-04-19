use super::*;

/// Topology defines the structure of spacetime, in particular how space and
/// time are quantized.
///
/// Any calculation which requires converting from absolute coordinates to
/// quantized coordinates must refer to the topology. Therefore, this type is
/// ubiquitous! More functions than not take it as a parameter. This may seem
/// cumbersome, but there are a few reasons why this is helpful:
/// - We currently use a "standard" quantization for all networks, but we may
///   find it beneficial in the future to let each network specify its own
///   quantization levels, based on its own traffic and longevity needs.
/// - It is confusing to be working with three different coordinate systems in
///   this codebase, and the presence of a `&topo` param in a function is a
///   helpful reminder to be extra mindful about the unit conversions that are
///   happening
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Topology {
    /// The quantization of space
    pub space: Dimension,
    /// The quantization of time
    pub time: Dimension,
    /// The origin of time, meaning the 0th quantum contains this Timestamp.
    pub time_origin: Timestamp,
}

impl Topology {
    /// Unit dimensions with the given time origin
    #[cfg(feature = "test_utils")]
    pub fn unit(time_origin: Timestamp) -> Self {
        Self {
            space: Dimension::unit(),
            time: Dimension::unit(),
            time_origin,
        }
    }

    /// Unit dimensions with a zero time origin
    #[cfg(feature = "test_utils")]
    pub fn unit_zero() -> Self {
        Self {
            space: Dimension::unit(),
            time: Dimension::unit(),
            time_origin: Timestamp::from_micros(0),
        }
    }

    /// Standard dimensions with the given time origin
    pub fn standard(time_origin: Timestamp) -> Self {
        Self {
            space: Dimension::standard_space(),
            time: Dimension::standard_time(),
            time_origin,
        }
    }

    /// Standard dimensions with the [`HOLOCHAIN_EPOCH`] as the time origin
    pub fn standard_epoch() -> Self {
        Self::standard(Timestamp::HOLOCHAIN_EPOCH)
    }

    /// Standard dimensions with a zero time origin
    #[cfg(feature = "test_utils")]
    pub fn standard_zero() -> Self {
        Self::standard(Timestamp::ZERO)
    }

    /// Returns the space quantum which contains this location
    pub fn space_quantum(&self, x: Loc) -> SpaceQuantum {
        (x.as_u32() / self.space.quantum).into()
    }

    /// Returns the time quantum which contains this timestamp
    pub fn time_quantum(&self, t: Timestamp) -> TimeQuantum {
        let t = (t.as_micros() - self.time_origin.as_micros()).max(0);
        ((t / self.time.quantum as i64) as u32).into()
    }

    /// Returns the time quantum which contains this timestamp
    pub fn time_quantum_duration(&self, d: std::time::Duration) -> TimeQuantum {
        ((d.as_micros() as i64 / self.time.quantum as i64) as u32).into()
    }

    /// The minimum power to use in "exponentional coordinates".
    pub fn min_space_power(&self) -> u8 {
        // if space quantum power is 0, then min has to be at least 1.
        // otherwise, it can be 0
        1u8.saturating_sub(self.space.quantum_power)
    }

    /// The maximum power to use in "exponentional coordinates".
    /// This is 17 for standard space topology. (32 - 12 - 3)
    pub fn max_space_power(&self, strat: &ArqStrat) -> u8 {
        32 - self.space.quantum_power - strat.max_chunks_log2()
    }
}

/// Defines the quantization of a dimension of spacetime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dimension {
    /// The smallest possible length in this dimension.
    /// Determines the interval represented by the leaf of a tree.
    pub quantum: u32,

    /// The smallest power of 2 which is larger than the quantum.
    /// Needed for various calculations.
    pub quantum_power: u8,

    /// The log2 size of this dimension, so that 2^bit_depth is the number of
    /// possible values that can be represented.
    pub(super) bit_depth: u8,
}

impl Dimension {
    /// No quantization.
    /// Used for testing, making it easier to construct values without thinking
    /// of unit conversions.
    #[cfg(feature = "test_utils")]
    pub fn unit() -> Self {
        Dimension {
            quantum: 1,
            quantum_power: 0,
            bit_depth: 32,
        }
    }

    /// The standard space quantum size is 2^12
    pub const fn standard_space() -> Self {
        let quantum_power = 12;
        Dimension {
            // if a network has 1 million peers,
            // the average spacing between them is ~4,300
            // so at a target coverage of 100,
            // each arc will be ~430,000 in length
            // which divided by 16 (max chunks) is ~2700, which is about 2^15.
            // So, we'll go down to 2^12 just to be extra safe.
            // This means we only need 20 bits to represent any location.
            quantum: 2u32.pow(quantum_power as u32),
            quantum_power,
            bit_depth: 32 - quantum_power,
        }
    }

    /// The standard time quantum size is 5 minutes (300 million microseconds)
    pub const fn standard_time() -> Self {
        Dimension {
            // 5 minutes in microseconds = 1mil * 60 * 5 = 300,000,000
            // log2 of this is 28.16, FYI
            quantum: 1_000_000 * 60 * 5,
            quantum_power: 29,

            // 12 quanta = 1 hour.
            // If we set the max lifetime for a network to ~100 years, which
            // is 12 * 24 * 365 * 1000 = 105,120,000 time quanta,
            // the log2 of which is 26.64,
            // then we can store any time coordinate in that range using 27 bits.
            //
            // BTW, the log2 of 100 years in microseconds is 54.81
            bit_depth: 27,
        }
    }
}

/// Node-specific parameters for gossip.
/// While the [`Topology`] must be the same for every node in a network, each
/// node is free to choose its own GossipParams.
///
/// Choosing smaller values for these offsets can lead to less resource usage,
/// at the expense of reducing opportunities to gossip with other nodes.
/// This is also largely dependent on the characteristcs of the network,
/// since if almost all nodes are operating with the same current timestamp
/// and Arq power level, there will be very little need for reconciliation.
///
/// In networks where nodes are offline for long periods of time, or latency
/// is very high (sneakernet), it could be helpful to increase these values.
#[derive(Copy, Clone, Debug, derive_more::Constructor)]
pub struct GossipParams {
    /// What +/- coordinate offset will you accept for timestamps?
    /// e.g. if the time quantum is 5 min,
    /// a time buffer of 2 will allow +/- 10 min.
    pub max_time_offset: TimeQuantum,

    /// What difference in power will you accept for other agents' Arqs?
    /// e.g. if the power I use in my arq is 16, and this offset is 2,
    /// I won't talk to anyone whose arq is expressed with a power lower
    /// than 14 or greater than 18.
    pub max_space_power_offset: u8,
}

impl GossipParams {
    /// Zero-tolerance gossip params
    pub fn zero() -> Self {
        Self {
            max_time_offset: 0.into(),
            max_space_power_offset: 0,
        }
    }
}
