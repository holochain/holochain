//! A RegionSet is a compact representation of a set of Regions -- the [`RegionCoords`]
//! defining the regions, and their associated [`RegionData`].
//!
//! Currently we have only one scheme for specifying RegionSets, called "LTCS".
//! "LTCS" is an acronym standing for "Logarithmic Time, Constant Space",
//! and it refers to our current scheme of partitioning spacetime during gossip, which
//! is to use a constant number of SpaceSegments (8..16) and a logarithmically
//! growing number of TimeSegments, with larger segments to cover older times.
//! In the future we may have other schemes.

mod ltcs;

pub use ltcs::*;

use crate::{error::GossipResult, spacetime::*};

use crate::region::{Region, RegionBounds, RegionCoords, RegionData, RegionDataConstraints};

/// The generic definition of a set of Regions.
/// The current representation is very specific to our current algorithm,
/// but this is an enum to make room for a more generic representation, e.g.
/// a simple Vec<Region>, if we want a more intricate algorithm later.
#[derive(Debug, derive_more::From)]
#[cfg_attr(feature = "test_utils", derive(Clone))]
pub enum RegionSet<T: RegionDataConstraints = RegionData> {
    /// Logarithmic Time, Constant Space.
    Ltcs(RegionSetLtcs<T>),
}

impl<D: RegionDataConstraints> RegionSet<D> {
    /// The number of regions specified
    pub fn count(&self) -> usize {
        match self {
            Self::Ltcs(set) => set.count(),
        }
    }

    /// Iterator over all Regions
    pub fn regions(&self) -> impl Iterator<Item = Region<D>> + '_ {
        match self {
            Self::Ltcs(set) => set.regions(),
        }
    }

    /// The RegionSet can be used to answer questions about more regions than
    /// just the ones specified: If a larger region is queried, and this set contains
    /// a set of regions which over that larger region, then the larger region
    /// can be dynamically constructed.
    ///
    /// This allows agents with differently computed RegionSets to still engage
    /// in gossip without needing to recompute regions.
    pub fn query(&self, _bounds: &RegionBounds) -> ! {
        unimplemented!("only implement after trying naive database-only approach")
    }

    /// In order for this RegionSet to be queryable, new data needs to be
    /// integrated into it to avoid needing to recompute it from the database
    /// on each query.
    pub fn update(&self, _c: SpacetimeQuantumCoords, _d: D) -> ! {
        unimplemented!("only implement after trying naive database-only approach")
    }

    /// Find a set of Regions which represents the intersection of the two
    /// input RegionSets.
    pub fn diff(self, other: Self) -> GossipResult<Vec<Region<D>>> {
        match (self, other) {
            (Self::Ltcs(left), Self::Ltcs(right)) => left.diff(right),
        }
        // Notes on a generic algorithm for the diff of generic regions:
        // can we use a Fenwick tree to look up regions?
        // idea:
        // sort the regions by power (problem, there are two power)
        // lookup the region to see if there's already a direct hit (most efficient if the sorting guarantees that larger regions get looked up later)
        // PROBLEM: we *can't* resolve rectangles where one is not a subset of the other
    }
}

#[cfg(feature = "test_utils")]
impl RegionSet {
    /// Return only the regions which have ops in them. Useful for testing
    /// sparse scenarios.
    pub fn nonzero_regions(
        &self,
    ) -> impl '_ + Iterator<Item = ((usize, usize, usize), RegionCoords, RegionData)> {
        match self {
            Self::Ltcs(set) => set.nonzero_regions(),
        }
    }
}

#[cfg(test)]
#[cfg(feature = "test_utils")]
mod tests {

    use kitsune_p2p_timestamp::Timestamp;

    use crate::{
        op::*,
        persistence::*,
        prelude::{ArqBoundsSet, ArqLocated, ArqStart},
        test_utils::{Op, OpData, OpStore},
        Arq, ArqBounds, Loc,
    };

    use super::*;

    /// Create a uniform grid of ops:
    /// - one gridline per arq segment
    /// - one gridline per time specified in the iterator
    ///
    /// Only works for arqs that don't span `u32::MAX / 2`
    fn op_grid<S: ArqStart>(
        topo: &Topology,
        arq: &Arq<S>,
        trange: impl Iterator<Item = i64> + Clone,
    ) -> Vec<Op> {
        let (left, right) = arq.to_edge_locs(topo);
        let left = left.as_u32();
        let right = right.as_u32();
        let mid = u32::MAX / 2;
        assert!(
            !(left < mid && right > mid),
            "This hacky logic does not work for arqs which span `u32::MAX / 2`"
        );
        let xstep = (arq.absolute_length(topo) / arq.count() as u64) as usize;
        (left as i32..=right as i32)
            .step_by(xstep)
            .flat_map(|x| {
                trange.clone().map(move |t| {
                    let x = SpaceQuantum::from(x as u32).to_loc_bounds(topo).0;
                    let t = TimeQuantum::from(t as u32).to_timestamp_bounds(topo).0;
                    OpData::fake(x, t, 10)
                })
            })
            .collect()
    }

    #[test]
    fn test_count() {
        use num_traits::Zero;
        let arqs = ArqBoundsSet::new(vec![
            ArqBounds::new(12, 11.into(), 8.into()),
            ArqBounds::new(12, 11.into(), 7.into()),
            ArqBounds::new(12, 11.into(), 5.into()),
        ]);
        let tt = TelescopingTimes::new(TimeQuantum::from(11));
        let nt = tt.segments().len();
        let expected = (8 + 7 + 5) * nt;
        let coords = RegionCoordSetLtcs::new(tt, arqs);
        assert_eq!(coords.count(), expected);
        let regions = coords.into_region_set_infallible(|_| RegionData::zero());
        assert_eq!(regions.count(), expected);
    }

    #[test]
    fn test_regions() {
        let topo = Topology::unit(Timestamp::from_micros(1000));
        let pow = 8;
        let arq = Arq::new(pow, 0u32.into(), 4.into());
        assert_eq!(
            arq.to_edge_locs(&topo),
            (Loc::from(0u32), Loc::from(1023u32))
        );

        let mut store = OpStore::new(topo.clone(), GossipParams::zero());

        // Create a nx by nt grid of ops and integrate into the store
        let nx = 8;
        let nt = 10;
        let ops = op_grid(
            &topo,
            &ArqLocated::new(pow, 0u32.into(), 8.into()),
            (1000..11000 as i64).step_by(1000),
        );
        assert_eq!(ops.len(), nx * nt);
        store.integrate_ops(ops.into_iter());

        // Calculate region data for all ops.
        // The total count should be half of what's in the op store,
        // since the arq covers exactly half of the ops
        let times = TelescopingTimes::new(TimeQuantum::from(11000));
        let coords = RegionCoordSetLtcs::new(times, ArqBoundsSet::single(arq.to_bounds(&topo)));
        let rset = RegionSetLtcs::from_store(&store, coords);
        assert_eq!(
            rset.data
                .concat()
                .concat()
                .iter()
                .map(|r| r.count)
                .sum::<u32>() as usize,
            nx * nt / 2
        );
    }

    #[test]
    fn test_rectify() {
        let topo = Topology::unit_zero();
        let arq = Arq::new(8, 0u32.into(), 4.into()).to_bounds(&topo);
        let mut store = OpStore::new(topo.clone(), GossipParams::zero());
        store.integrate_ops(op_grid(&topo, &arq, 10..20).into_iter());

        let tt_a = TelescopingTimes::new(TimeQuantum::from(20));
        let tt_b = TelescopingTimes::new(TimeQuantum::from(30));
        let coords_a = RegionCoordSetLtcs::new(tt_a, ArqBoundsSet::single(arq.clone()));
        let coords_b = RegionCoordSetLtcs::new(tt_b, ArqBoundsSet::single(arq.clone()));

        let mut rset_a = RegionSetLtcs::from_store(&store, coords_a);
        let mut rset_b = RegionSetLtcs::from_store(&store, coords_b);
        assert_ne!(rset_a.data, rset_b.data);

        rset_a.rectify(&mut rset_b).unwrap();

        assert_eq!(rset_a, rset_b);

        let coords: Vec<Vec<_>> = rset_a
            .coords
            .region_coords_nested()
            .map(|col| col.collect())
            .collect();

        assert_eq!(coords.len(), arq.count() as usize);
        for col in coords.iter() {
            assert_eq!(col.len(), rset_a.coords.times.segments().len());
        }
        let nt = coords[0].len();
        assert_eq!(tt_b.segments()[0..nt], rset_a.coords.times.segments());
        assert_eq!(tt_b.segments()[0..nt], rset_b.coords.times.segments());
    }

    #[test]
    fn test_diff() {
        let topo = Topology::unit_zero();
        let arq = Arq::new(8, Loc::from(-512i32 as u32), 4.into()).to_bounds(&topo);
        dbg!(&arq, arq.to_dht_arc_range(&topo));

        let mut store1 = OpStore::new(topo.clone(), GossipParams::zero());
        store1.integrate_ops(op_grid(&topo, &arq, 10..20).into_iter());

        let extra_ops = [
            OpData::fake(Loc::from(-300i32), Timestamp::from_micros(18), 4),
            OpData::fake(Loc::from(12u32), Timestamp::from_micros(12), 4),
        ];
        let mut store2 = store1.clone();
        store2.integrate_ops(extra_ops.clone().into_iter());

        let coords_a = RegionCoordSetLtcs::new(
            TelescopingTimes::new(TimeQuantum::from(20)),
            ArqBoundsSet::single(arq.clone()),
        );
        let coords_b = RegionCoordSetLtcs::new(
            TelescopingTimes::new(TimeQuantum::from(21)),
            ArqBoundsSet::single(arq.clone()),
        );

        let rset_a = RegionSetLtcs::from_store(&store1, coords_a);
        let rset_b = RegionSetLtcs::from_store(&store2, coords_b);
        assert_ne!(rset_a.data, rset_b.data);

        let diff = rset_a.clone().diff(rset_b.clone()).unwrap();
        dbg!(&diff, &extra_ops);
        assert_eq!(diff.len(), 2);

        assert!(diff[0].coords.contains(&topo, &extra_ops[0].coords(&topo)));
        assert!(diff[1].coords.contains(&topo, &extra_ops[1].coords(&topo)));

        // Adding the region data from each extra op to the region data of the
        // diff which was missing those ops should be the same as the query
        // of the store which contains the extra ops over the same region
        // TODO: proptest this
        assert_eq!(
            diff[0].data + extra_ops[0].region_data(),
            store2.query_region_data(&diff[0].coords)
        );
        assert_eq!(
            diff[1].data + extra_ops[1].region_data(),
            store2.query_region_data(&diff[1].coords)
        );
    }

    #[test]
    fn test_diff_standard_topo() {
        let topo = Topology::standard_zero();
        let pow: u8 = 4;
        // This arq goes from -2^17 to 2^17, with a chunk size of 2^16
        let left_edge = Loc::from(-(2i32.pow(pow as u32 + 12 + 1)));
        let arq = Arq::new(pow, left_edge, 4.into()).to_bounds(&topo);
        dbg!(&arq, arq.to_dht_arc_range(&topo));

        let mut store1 = OpStore::new(topo.clone(), GossipParams::zero());
        store1.integrate_ops(op_grid(&topo, &arq, 10..20).into_iter());

        let extra_ops = [
            OpData::fake(
                left_edge,
                TimeQuantum::from(18).to_timestamp_bounds(&topo).0,
                13,
            ),
            OpData::fake(
                Loc::from(11111u32),
                TimeQuantum::from(12).to_timestamp_bounds(&topo).0,
                11,
            ),
        ];
        // Store 2 has everything store 1 has, plus 2 extra ops
        let mut store2 = store1.clone();
        store2.integrate_ops(extra_ops.clone().into_iter());

        let coords_a = RegionCoordSetLtcs::new(
            TelescopingTimes::new(TimeQuantum::from(20)),
            ArqBoundsSet::single(arq.clone()),
        );
        let coords_b = RegionCoordSetLtcs::new(
            TelescopingTimes::new(TimeQuantum::from(21)),
            ArqBoundsSet::single(arq.clone()),
        );

        let rset_a = RegionSetLtcs::from_store(&store1, coords_a);
        let rset_b = RegionSetLtcs::from_store(&store2, coords_b);
        assert_ne!(rset_a.data, rset_b.data);

        let diff = rset_a.clone().diff(rset_b.clone()).unwrap();
        dbg!(&diff, &extra_ops);
        assert_eq!(diff.len(), 2);

        assert!(diff[0].coords.contains(&topo, &extra_ops[0].coords(&topo)));
        assert!(diff[1].coords.contains(&topo, &extra_ops[1].coords(&topo)));

        // Adding the region data from each extra op to the region data of the
        // diff which was missing those ops should be the same as the query
        // of the store which contains the extra ops over the same region
        // TODO: proptest this
        assert_eq!(
            diff[0].data + extra_ops[0].region_data(),
            store2.query_region_data(&diff[0].coords)
        );
        assert_eq!(
            diff[1].data + extra_ops[1].region_data(),
            store2.query_region_data(&diff[1].coords)
        );
    }
}
