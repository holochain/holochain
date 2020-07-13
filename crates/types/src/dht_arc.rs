//! A type for indicating ranges on the dht arc

use derive_more::From;
use std::num::Wrapping;

#[derive(Debug, Clone, Copy, PartialEq, Eq, From)]
/// Type for representing a location that can wrap around
/// a u32 dht arc
pub struct Location(pub Wrapping<u32>);

/// The maximum you can hold either side of the hash location
/// is half te circle
pub const MAX_LENGTH: i64 = u32::MAX as i64 / 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Represents how much of a dht arc is held
/// hash_location is where the hash is.
/// The hash_location is the center of the arc
/// length -1 means nothing is held
/// length 0 means just the hash_location is held
/// length n where n > 0 means n locations are
/// held on either side of hash_location 
pub struct DhtArc {
    hash_location: Location,
    length_either_side: i64,
}

impl DhtArc {
    /// Create an Arc from a hash location plus a length on either side
    /// Length is (0..u32::Max + 1)
    pub fn new<I: Into<Location>>(hash_location: I, length_either_side: i64) -> Self {
        let length_either_side = std::cmp::max(length_either_side, -1);
        let length_either_side = std::cmp::min(length_either_side, MAX_LENGTH);
        Self {
            hash_location: hash_location.into(),
            length_either_side,
        }
    }

    /// Check if a location is contained in this arc
    pub fn contains<I: Into<Location>>(&self, location: I) -> bool {
        self.length_either_side >= 0
            && i64::from(shortest_arc_distance(self.hash_location, location.into()))
                <= self.length_either_side
    }
}

impl From<u32> for Location {
    fn from(a: u32) -> Self {
        Self(Wrapping(a))
    }
}

/// Finds the shortest absolute distance between two points on a circle
fn shortest_arc_distance<A: Into<Location>, B: Into<Location>>(a: A, b: B) -> u32 {
    // Turn into wrapped u32s
    let a = a.into().0;
    let b = b.into().0;
    std::cmp::min(a - b, b - a).0
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: This is a really good place for prop testing

    #[test]
    fn test_arc_dist() {
        // start at 5 go all the way around the arc anti-clockwise until
        // you reach 5. You will have traveled 5 less then the entire arc plus one
        // for the reserved zero value
        assert_eq!(shortest_arc_distance(10, 5), 5);
        assert_eq!(shortest_arc_distance(5, 10), 5);
        assert_eq!(
            shortest_arc_distance(Wrapping(u32::MAX) + Wrapping(5), u32::MAX),
            5
        );
        assert_eq!(shortest_arc_distance(0, u32::MAX), 1);
    }

    #[test]
    fn test_dht_arc() {
        assert!(!DhtArc::new(0, -1).contains(0));

        assert!(DhtArc::new(0, 0).contains(0));

        assert!(!DhtArc::new(0, 0).contains(1));
        assert!(!DhtArc::new(1, -1).contains(0));
        assert!(!DhtArc::new(1, -1).contains(1));

        assert!(DhtArc::new(1, 0).contains(1));
        assert!(DhtArc::new(0, 1).contains(0));
        assert!(DhtArc::new(0, 1).contains(1));
        assert!(DhtArc::new(0, 1).contains(u32::MAX));

        assert!(!DhtArc::new(0, 1).contains(2));
        assert!(!DhtArc::new(0, 1).contains(3));
        assert!(!DhtArc::new(0, 1).contains(u32::MAX - 1));
        assert!(!DhtArc::new(0, 1).contains(u32::MAX - 2));

        assert!(DhtArc::new(0, 2).contains(2));
        assert!(DhtArc::new(0, 2).contains(u32::MAX - 1));
    }
}
