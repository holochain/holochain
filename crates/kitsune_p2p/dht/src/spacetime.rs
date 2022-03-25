//! Data types representing the structure of spacetime and the various ways
//! space and time can be quantized.
//!
//! Kitsune deals with spacetime coordinates on three different levels:
//!
//! ### Absolute coordinates
//!
//! At the absolute level, space coordinates are represented by `u32` (via `DhtLocation`),
//! and time coordinates by `i64` (via `Timestamp`). The timestamp and DHT location
//! of each op is measured in absolute coordinates, as well as the DHT locations of
//! agents
//!
//! ### Quantized coordinates
//!
//! Some data types represent quantized space/time. The `Topology` for a network
//! determines the quantum size for both the time and space dimensions, meaning
//! that any absolute coordinate will always be a multiple of this quantum size.
//! Hence, quantized coordinates are expressed in terms of multiples of the
//! quantum size.
//!
//! `SpaceQuantum` and `TimeQuantum` express quantized coordinates. They refer
//! to a specific quantum-sized portion of space/time.
//!
//! Note that any transformation between Absolute and Quantized coordinates
//! requires the information contained in the `Topology` of the network.
//!
//! ### Segment coordinates (or, Exponential coordinates)
//!
//! The spacetime we are interested in has dimensions that are not only quantized,
//! but are also hierarchically organized into non-overlapping segments.
//! When expressing segments of space larger than a single quantum, we only ever talk about
//! groupings of 2, 4, 8, 16, etc. quanta at a time, and these groupings are
//! always aligned so that no two segments of a given size ever overlap. Moreover,
//! any two segments of different sizes either overlap completely (one is a strict
//! superset of the other), or they don't overlap at all (they are disjoint sets).
//!
//! Segment coordinates are expressed in terms of:
//! - a *power* (exponent of 2) which determines the length of the segment *expressed as a Quantized coordinate*
//! - an *offset*, which is a multiple of the length of this segment to determine
//!   the "left" edge's distance from the origin *as a Quantized coordinate*
//!
//! You must still convert from these Quantized coordinates to get to the actual
//! Absolute coordinates.
//!
//! The pairing of any `SpaceSegment` with any `TimeSegment` forms a `Region`,
//! a bounded rectangle of spacetime.
//!

use std::ops::{AddAssign, Deref};

use crate::{
    op::{Loc, Timestamp},
    prelude::pow2,
    ArqStrat,
};
use derivative::Derivative;

mod quantum;
mod segment;
mod telescoping_times;
mod topology;

pub use quantum::*;
pub use segment::*;
pub use telescoping_times::*;
pub use topology::*;
