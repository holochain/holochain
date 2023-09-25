use std::num::Wrapping;

use crate::{arq::*, spacetime::Topology};

/// Margin of error for floating point comparisons
const ERROR_MARGIN: f64 = 0.0000000001;

/// Check a set of peers the actual redundancy across all peers.
/// This can tell if there is bad distribution.
/// Note this function is only used for verification in tests at this time.
pub fn calc_min_redundancy(topo: &Topology, peers: Vec<Arq>) -> u32 {
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

    // Turn each arc into a side with a unique id that is
    // shared by both sides.
    let mut id = 0;
    let mut sides = |arq: &Arq| {
        let (left, right) = arq.to_edge_locs(topo);
        let i = id;
        let l = Arm {
            id: i,
            side: Side::Left,
            pos: left.as_u32(),
        };
        let r = Arm {
            id: i,
            side: Side::Right,
            pos: right.as_u32(),
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
            if (a.coverage(topo) - 1.0).abs() < ERROR_MARGIN {
                full_r += 1;
                false
            } else {
                // Also remove any bounds that don't include some coverage.
                !a.is_empty()
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
    r as u32 + full_r
}
