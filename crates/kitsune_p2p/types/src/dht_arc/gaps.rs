//! Functions for checking gaps in coverage for tests.

use super::*;

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

/// Check a set of peers the actual redundancy across all peers.
/// This can tell if there is bad distribution.
/// Note this function is only used for verification in tests at this time.
pub fn check_redundancy(peers: Vec<DhtArc>) -> usize {
    use std::collections::HashSet;
    #[derive(Clone, Copy, Debug)]
    enum Side {
        Left,
        Right,
    }
    #[derive(Clone, Copy, Debug)]
    struct Arm {
        id: usize,
        side: Side,
        pos: u32,
    }
    let left = |arc: &DhtArc| match arc.range().start_bound() {
        Bound::Included(arm) => *arm,
        _ => unreachable!(),
    };
    let right = |arc: &DhtArc| match arc.range().end_bound() {
        Bound::Included(arm) => *arm,
        _ => unreachable!(),
    };

    // Turn each arc into a side with a unique id that is
    // shared by both sides.
    let mut id = 0;
    let mut sides = |arc: &DhtArc| {
        let i = id;
        let l = Arm {
            id: i,
            side: Side::Left,
            pos: left(arc),
        };
        let r = Arm {
            id: i,
            side: Side::Right,
            pos: right(arc),
        };
        id += 1;
        vec![l, r]
    };

    // Record and remove any full redundancy arcs as we only
    // need to measure that stack of partial coverage.
    let mut full_r = 0;
    let peers: Vec<_> = peers
        .into_iter()
        .filter(|a| {
            if (a.coverage() - 1.0).abs() < ERROR_MARGIN {
                full_r += 1;
                false
            } else {
                // Also remove any bounds that don't include some coverage.
                matches!(a.range().start_bound(), Bound::Included(_))
            }
        })
        .collect();

    // If we are empty at this stage then return any full coverage.
    if peers.is_empty() {
        return full_r;
    }

    // Turn the rest of the partial arcs into their sides.
    let mut peers = peers
        .into_iter()
        .flat_map(|p| sides(&p).into_iter())
        .collect::<Vec<_>>();

    // Sort the sides by their positions.
    peers.sort_unstable_by_key(|p| p.pos);

    // Fold over the sides tracking the stack of arcs that have been entered but not exited.
    // The minimal stack height at any given point is the minimum redundancy on the network.
    let stack_fold = |(mut stack, r, mut started, mut last_remove): (
        HashSet<usize>,
        usize,
        bool,
        Option<u32>,
    ),
                      arm: &Arm| {
        let mut connected = false;
        let mut this_remove = None;
        match arm.side {
            Side::Left => {
                // We must have added at least one arc otherwise
                // our minimum stack height will always be one.
                started = true;

                // Check if we are inserting an arc just one location
                // past a remove because that actually counts as covered.
                connected = last_remove
                    .as_ref()
                    .map(|l| (Wrapping(arm.pos) - Wrapping(*l)).0 <= 1)
                    .unwrap_or(false);

                // Add this id to the stack.
                stack.insert(arm.id);
            }
            Side::Right => {
                // Set the last removed.
                this_remove = Some(arm.pos);

                // Remove this id.
                stack.remove(&arm.id);
            }
        }
        // Get the current stack height.
        let len = stack.len();

        // If we have started and the length has dropped then set a
        // lower redundancy.
        let mut r = if len < r && started {
            // Only record removes that actually change the r level.
            last_remove = this_remove;
            len
        } else {
            r
        };

        // If this was actually a connected insert then undo the last remove.
        if connected {
            r += 1
        }
        (stack, r, started, last_remove)
    };

    // Run through the list once to find the stack remaining at the end of the run.
    let (stack, _, started, last_removed) = peers
        .iter()
        .fold((HashSet::new(), usize::MAX, false, None), stack_fold);

    // Now use that as the starting stack for the "real" run.
    let (_, r, _, _) = peers
        .iter()
        .fold((stack, usize::MAX, started, last_removed), stack_fold);

    // Our redundancy is whatever partial + any full redundancy
    r + full_r
}
