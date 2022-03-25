//! Defines the structure of the DHT, the objects which inhabit it, and the operations
//! on these objects.
//!
//! ## Structure and contents of the DHT
//!
//! The DHT is populated by Agents and Ops. Agents introduce new Ops to the DHT,
//! and are responsible for storing their own Ops and the Ops of other Agents.
//!
//! Each Agent has a fixed Location, and each Op has both a fixed
//! Location and a fixed Timestamp.
//!
//! Locations are one-dimensional, and are best thought of as positions
//! on a circle. They are represented by `Wrapping<u32>`.
//! Timestamps are best thought of as positions on a timeline, and are represented
//! by an i64 which denotes microseconds since the UNIX epoch.
//! The Location dimension is circular, and the Time dimension is linear.
//!
//! Each Agent has an Arc which marks out a portion of the circle of Locations.
//! Arcs have a fixed starting point, and extends "clockwise" by some length which
//! is chosen by Kitsune and changes over time. Every Agent is responsible for
//! holding a copy of any Ops whose location overlaps their Arc. As we will see
//! later, Arcs are *quantized*.
//!
//! ### Geometrical interpretation
//!
//! This crate works heavily with the concept of a two-dimensional cylindrical
//! "spacetime" surface -- all terminology in this crate is built with this
//! model in mind, so it helps to explain it a bit here.
//!
//! If we think of Location as a spatial dimension,
//! and Time as a temporal dimension, we can think of the DHT as a
//! two-dimensional *spacetime*.
//!
//! The Location dimension is circular, i.e. the endpoints are connected,
//! but the Time dimension is linear and constantly expanding. So, these two
//! dimensions form a cylindrical surface, with its base at the origin time of
//! the network, and a constantly increasing height as time marches forward.
//!
//! So:
//! - An Op can be thought of a point somewhere on the surface of this cylinder,
//! - An Agent can be thought of as a line perpendicular to the base, extending
//!   across the entire length of the cylinder.
//! - An Arcs can be thought of as a rectangular "stripe" made by sweeping the
//!   Agent line across the distance specified by the length of the Arc.
//! - A *Region* is an arbitrary rectangular area of spacetime.
//!
//! ## Quantization
//!
//! One key feature of this DHT space is that it is *quantized* in both dimensions.
//! You can think of this as a grid which overlays spacetime, which has the effect
//! of grouping Agents and Ops into buckets specified by the grid cells. Another
//! way to think of this is we only refer to items by which grid cell they appear in,
//! not by their absolute coordinates.
//!
//! ### Topology and Quantum Coordinates
//!
//! The quantization is specified by a parameter called the [`Topology`]. This
//! construct defines how the quantized grid is overlaid onto spacetime, and specifies
//! how to transform between "absolute" coordinates (Locations and Timestamps) and
//! "quantum" coordinates (grid cells).
//!
//! Each Kitsune Space is free to choose its own Topology (and all nodes in that space
//! must agree to use the same Topology). However, in practice we currently use the same
//! topology for all spaces:
//! - the standard space quantum size is 2^12
//! - the standard time quantum size is 5 minutes
//!
//! Additionally, we include a "time origin" component in the topology, which describes
//! a shift of the origin of the time dimension of the quantized grid. In other words,
//! the time origin specifies what Timestamp corresponds to the origin coordinate of
//! the grid's time dimension.
//!
//! For the purpose of terminology, we refer to these rectangular grid cells as the
//! spacetime quanta. The sides of these rectangles are the space quanta and time quanta.
//!
//! ### Segments (Chunks) and Exponential Coordinates (TODO land on a name for this?)
//!
//! For the purposes of our gossip algorithm, we specify Arcs in terms of "segments"
//! or "chunks". A Segment is a set of `2^p` contiguous quanta, where `p` is a whole number.
//! So, segments are always a power-of-two multiple of the quantum size. Additionally, the offset
//! from the origin must also be a power of two. So, the set of all possible segments
//! implies a collection of uniform grids layered on top of the quantum grid, each one twice
//! as coarse as the one below it.
//!
//! Thus, we can refer to the successive levels of coarseness of quantization by a single
//! whole number, referred to as the "power", since it takes the form of `2^p`.
//! A "power" of 0 refers to the quantum grid itself. A power of 1 is twice as coarse, and so on.
//!
//! Arcs are specified as a set of contiguous Segments all of the same power.
//!
//! ## Quantized Gossip
//!
//! The reason for this detailed quantization of spacetime is to implement historical gossip between
//! agents efficiently. Each Agent has its own Arc which represents which Ops it is responsible
//! for storing and gossiping to other agents. When two agents gossip, they have to quickly and
//! efficiently determine which Ops they already have in common, and which they need to send
//! to each other.
//!
//! This as accomplished by agents exchanging information about the Regions of spacetime that they
//! hold in common. This needs to be done with a minimum of coordination, which is why we use a
//! uniform quantization of spacetime -- if there are only so many ways to split up spacetime into
//! Regions, there is a greater chance that agents will have information on the exact same regions.
//!
//! Concretely, when agents gossip, they exchange the "fingerprint" of each region they have in common
//! with their counterpart, and for any regions whose fingerprints mismatch, each agent sends all
//! the ops in that region to their counterpart. For the regions which match, it is understood that
//! both parties hold the same data in those regions and need no further gossip on those regions.
//! Therefore, it is important to split spacetime into regions in such a way that when mismatches in
//! region fingerprints occur, they are for small amounts of data; or if they are for large amounts
//! of data, then the mismatches should be rare. At the same time, we don't want to split into too
//! many regions, because that would lead to a larger overhead with minimal gains.
//!
//! ### Quantizing space
//!
//! Each agent selects a power level for its arc based on its observation of the network conditions.
//! Ideally, each agent sees a similar view of the network and chooses the same power level as the
//! other nearby agents. When the power levels match, the two agents are tracking space at the same
//! level of granularity and can more easily coordinate. Inasmuch as the power levels differ,
//! one of the agents will have to do some extra computation to reconcile the difference.
//!
//! Without going into too much detail, the power level is mainly a function of how many other
//! Agents are in the vicinity. Agents start with a high power level, covering large sections of
//! the DHT with a coarsely subdivided arc, and as more agents join, they reduce their power level,
//! having smaller segments and tracking ops at a finer level of resolution.
//!
//! With the standard parameters for Arc resizing, it is guaranteed that an agent's Arc will always
//! contain between 8 and 15 segments.
//!
//! ### Quantizing time
//!
//! There is also the time dimension to consider. Agents subdivide space according to who else is
//! around. They subdivide time differently, and somewhat more simply: the older a particular
//! time is, the larger the time segment it will be a part of. The rationale for this is that
//! older data experiences changes far less often than newer data, so it is very likely for
//! older regions to be similar between two agents, and thus they can compare older data in larger
//! swathes than newer data, which is expected to experience changes more frequently.
//!
//! This also has the nice property that the number of segments we use in time only increases
//! logarithmically with the passing of time itself, since older regions are twice as long
//! in the time dimension as newer regions.
//!
//! Since the number of space segments in an arc has constant upper and lower bounds, and the
//! number of time segments increases logarithmically, the overall overhead of coordination
//! during gossip increases only logarithmically as the network grows. Thus, even long-lived
//! networks have the ability to do historical gossip efficiently.

#![warn(missing_docs)]

pub mod arq;
pub mod error;
pub mod hash;
pub mod op;
pub mod region;
pub mod region_set;
pub mod spacetime;

pub use arq::{actual_coverage, Arq, ArqBounds, ArqStrat, PeerStrat, PeerView, PeerViewQ};

// The persistence traits are currently unused except for test implementations of
// a kitsune host. If we ever use them in actual host implementations, we can
// take the feature flag off.
#[cfg(feature = "test_utils")]
pub mod persistence;

#[cfg(feature = "test_utils")]
pub mod test_utils;

pub use kitsune_p2p_dht_arc::DhtLocation as Loc;

/// Common exports
pub mod prelude {
    pub use super::arq::*;
    pub use super::error::*;
    pub use super::hash::*;
    pub use super::op::*;
    pub use super::persistence::*;
    pub use super::region::*;
    pub use super::region_set::*;
    pub use super::spacetime::*;
}
