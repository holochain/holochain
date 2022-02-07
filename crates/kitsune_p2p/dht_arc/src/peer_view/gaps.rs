//! Functions for checking gaps in coverage for tests.

use crate::*;

use std::ops::{Bound, RangeBounds};

/// Check a set of peers for a gap in coverage.
/// Note this function is only used for verification in tests at this time.
pub fn check_for_gaps(peers: Vec<DhtArc>) -> bool {
    let left = |arc: &DhtArc| match arc.range().start_bound() {
        Bound::Included(arm) => *arm,
        _ => unreachable!(),
    };
    let mut peers: Vec<_> = peers
        .into_iter()
        .filter(|a| matches!(a.range().start_bound(), Bound::Included(_)))
        .collect();
    if peers.is_empty() {
        return true;
    }
    peers.sort_unstable_by_key(|p| left(p));
    if (peers[0].coverage() - 1.0).abs() < ERROR_MARGIN {
        return false;
    }
    // Translate the peers to zero to make wrapping checks easy.
    let translate = left(&peers[0]);
    // Safe to cast because of the coverage check
    let mut max = peers[0].range().len() as u32;

    for peer in peers {
        if (peer.coverage() - 1.0).abs() < ERROR_MARGIN {
            return false;
        }
        let l = left(&peer) - translate;
        if l > max {
            return true;
        }
        max = match l.checked_add(peer.range().len() as u32) {
            Some(m) => m,
            None => {
                // We reached the end and we know 0 is covered
                // so there is no gap.
                return false;
            }
        };
    }
    // Didn't reach the end
    true
}
