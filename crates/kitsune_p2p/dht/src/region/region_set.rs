use kitsune_p2p_timestamp::Timestamp;

use crate::{
    arq::*,
    coords::*,
    error::{GossipError, GossipResult},
    host::AccessOpStore,
    op::OpRegion,
    tree::TreeDataConstraints,
};

use super::{Region, RegionCoords, RegionData};

#[derive(Debug, PartialEq, Eq, derive_more::Constructor)]
#[cfg_attr(feature = "testing", derive(Clone))]
pub struct RegionCoordSetXtcs {
    times: TelescopingTimes,
    arq_set: ArqSet,
}

impl RegionCoordSetXtcs {
    /// Generate the XTCS region coords given the generating parameters.
    /// Each RegionCoords is paired with the relative spacetime coords, which
    /// can be used to pair the generated coords with stored data.
    pub fn region_coords_flat<'a>(
        &'a self,
        topo: &'a Topology,
    ) -> impl Iterator<Item = ((SpaceCoord, TimeCoord), RegionCoords)> + 'a {
        self.region_coords_nested(topo).flatten()
    }

    pub fn region_coords_nested<'a>(
        &'a self,
        topo: &'a Topology,
    ) -> impl Iterator<Item = impl Iterator<Item = ((SpaceCoord, TimeCoord), RegionCoords)>> + 'a
    {
        self.arq_set.arqs().iter().flat_map(move |arq| {
            arq.segments().enumerate().map(move |(ix, x)| {
                self.times
                    .segments()
                    .into_iter()
                    .enumerate()
                    .map(move |(it, t)| {
                        (
                            (SpaceCoord::from(ix as u32), TimeCoord::from(it as u32)),
                            RegionCoords::new(x, t),
                        )
                    })
            })
        })
    }

    pub fn empty() -> Self {
        Self {
            times: TelescopingTimes::empty(),
            arq_set: ArqSet::empty(11),
        }
    }
}

/// The generic definition of a set of Regions.
/// The current representation is very specific to our current algorithm,
/// but this is an enum to make room for a more generic representation, e.g.
/// a simple Vec<Region>, if we want a more intricate algorithm later.
#[derive(Debug, derive_more::From)]
#[cfg_attr(feature = "testing", derive(Clone))]
pub enum RegionSet<T: TreeDataConstraints = RegionData> {
    /// eXponential Time, Constant Space.
    Xtcs(RegionSetXtcs<T>),
}

/// Implementation for the compact XTCS region set format which gets sent over the wire.
/// The coordinates for the regions are specified by a few values.
/// The data to match the coordinates are specified in a 2D vector which must
/// correspond to the generated coordinates.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "testing", derive(Clone))]
pub struct RegionSetXtcs<D: TreeDataConstraints = RegionData> {
    /// The generator for the coordinates
    pub(crate) coords: RegionCoordSetXtcs,

    /// The outer vec corresponds to the spatial segments;
    /// the inner vecs are the time segments.
    pub(crate) data: Vec<Vec<D>>,
}

impl<D: TreeDataConstraints> RegionSetXtcs<D> {
    pub fn empty() -> Self {
        Self {
            coords: RegionCoordSetXtcs::empty(),
            data: vec![],
        }
    }

    pub fn new<O: OpRegion<D>, S: AccessOpStore<D, O>>(
        topo: &Topology,
        store: &S,
        coords: RegionCoordSetXtcs,
    ) -> Self {
        let data = coords
            .region_coords_nested(topo)
            .map(|columns| {
                columns
                    .map(|(_, coords)| store.query_region_coords(&coords))
                    .collect()
            })
            .collect();
        Self { coords, data }
    }

    pub fn count(&self) -> usize {
        if self.data.is_empty() {
            0
        } else {
            self.data.len() * self.data[0].len()
        }
    }

    pub fn regions<'a>(&'a self, topo: &'a Topology) -> impl Iterator<Item = Region<D>> + 'a {
        self.coords
            .region_coords_flat(topo)
            .map(|((ix, it), coords)| Region::new(coords, self.data[*ix as usize][*it as usize]))
    }

    /// Reshape the two region sets so that both match, omitting or merging
    /// regions as needed
    pub fn rectify(&mut self, other: &mut Self, topo: &Topology) -> GossipResult<()> {
        if self.coords.arq_set != other.coords.arq_set {
            return Err(GossipError::ArqSetMismatchForDiff);
        }
        if self.coords.times > other.coords.times {
            std::mem::swap(self, other);
        }
        let mut len = 0;
        for (da, db) in self.data.iter_mut().zip(other.data.iter_mut()) {
            TelescopingTimes::rectify((&self.coords.times, da), (&other.coords.times, db));
            len = da.len();
        }
        let times = other.coords.times.limit(len as u32);
        self.coords.times = times;
        other.coords.times = times;
        Ok(())
    }

    pub fn diff(mut self, mut other: Self, topo: &Topology) -> GossipResult<Vec<Region<D>>> {
        self.rectify(&mut other, topo)?;

        let regions = self
            .regions(topo)
            .into_iter()
            .zip(other.regions(topo).into_iter())
            .filter_map(|(a, b)| (a.data != b.data).then(|| a))
            .collect();

        Ok(regions)
    }
}

impl<D: TreeDataConstraints> RegionSet<D> {
    pub fn count(&self) -> usize {
        match self {
            Self::Xtcs(set) => set.count(),
        }
    }
    /// can be used to pair the generated coords with stored data.
    pub fn region_coords<'a>(
        &'a self,
        topo: &'a Topology,
    ) -> impl Iterator<Item = RegionCoords> + 'a {
        match self {
            Self::Xtcs(set) => set
                .coords
                .region_coords_flat(topo)
                .map(|(_, coords)| coords),
        }
    }

    /// Find a set of Regions which represents the intersection of the two
    /// input RegionSets.
    pub fn diff(self, other: Self, topo: &Topology) -> GossipResult<Vec<Region<D>>> {
        match (self, other) {
            (Self::Xtcs(left), Self::Xtcs(right)) => left.diff(right, topo),
        }
        // Notes on a generic algorithm for the diff of generic regions:
        // can we use a Fenwick tree to look up regions?
        // idea:
        // sort the regions by power (problem, there are two power)
        // lookup the region to see if there's already a direct hit (most efficient if the sorting guarantees that larger regions get looked up later)
        // PROBLEM: we *can't* resolve rectangles where one is not a subset of the other
    }
}

#[cfg(test)]
#[cfg(feature = "testing")]
mod tests {

    use crate::{
        op::{Op, OpData},
        test_utils::op_store::OpStore,
    };

    use super::*;

    /// Only works for arqs that don't span `u32::MAX / 2`
    fn op_grid(arq: &ArqBounds, trange: impl Iterator<Item = i32> + Clone) -> Vec<Op> {
        let (left, right) = (arq.left(), arq.right());
        let mid = u32::MAX / 2;
        assert!(
            !(left < mid && right > mid),
            "This hacky logic does not work for arqs which span `u32::MAX / 2`"
        );
        let xstep = (arq.length() / arq.count() as u64) as usize;
        (left as i32..arq.right() as i32 + 1)
            .step_by(xstep)
            .flat_map(|x| {
                trange.clone().map(move |t| {
                    // 16 x 100 total ops.
                    // x interval: [-1024, -1024)
                    // t interval: [1000, 11000)
                    OpData::fake(x as u32, t as i64, 10)
                })
            })
            .collect()
    }

    #[test]
    fn test_regions() {
        // (-512, 512)
        let pow = 8;
        let arq = Arq::new(0.into(), pow, 4).to_bounds();
        assert_eq!(arq.left() as i32, -512);
        assert_eq!(arq.right(), 511 as u32);

        let topo = Topology::identity(Timestamp::from_micros(1000));
        let mut store = OpStore::new(topo.clone(), GossipParams::zero());

        // Create a nx by nt grid of ops and integrate into the store
        let nx = 8;
        let nt = 10;
        let ops = op_grid(
            &Arq::new(0.into(), pow, 8).to_bounds(),
            (1000..11000 as i32).step_by(1000),
        );
        assert_eq!(ops.len(), nx * nt);
        store.integrate_ops(ops.into_iter());

        // Calculate region data for all ops.
        // The total count should be half of what's in the op store,
        // since the arq covers exactly half of the ops
        let times = topo.telescoping_times(Timestamp::from_micros(11000));
        let coords = RegionCoordSetXtcs::new(times, ArqSet::single(arq));
        let rset = RegionSetXtcs::new(&topo, &store, coords);
        assert_eq!(
            rset.data.concat().iter().map(|r| r.count).sum::<u32>() as usize,
            nx * nt / 2
        );
    }

    #[test]
    fn test_rectify() {
        let arq = Arq::new(0.into(), 8, 4).to_bounds();
        let topo = Topology::identity(Timestamp::from_micros(0));
        let mut store = OpStore::new(topo.clone(), GossipParams::zero());
        store.integrate_ops(op_grid(&arq, 10..20).into_iter());

        let tt_a = topo.telescoping_times(Timestamp::from_micros(20));
        let tt_b = topo.telescoping_times(Timestamp::from_micros(30));
        let coords_a = RegionCoordSetXtcs::new(tt_a, ArqSet::single(arq.clone()));
        let coords_b = RegionCoordSetXtcs::new(tt_b, ArqSet::single(arq.clone()));

        let mut rset_a = RegionSetXtcs::new(&topo, &store, coords_a);
        let mut rset_b = RegionSetXtcs::new(&topo, &store, coords_b);
        assert_ne!(rset_a.data, rset_b.data);

        rset_a.rectify(&mut rset_b, &topo).unwrap();

        assert_eq!(rset_a, rset_b);

        let coords: Vec<Vec<_>> = rset_a
            .coords
            .region_coords_nested(&topo)
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
        let arq = Arq::new(0.into(), 8, 4).to_bounds();
        let topo = Topology::identity(Timestamp::from_micros(0));

        let mut store1 = OpStore::new(topo.clone(), GossipParams::zero());
        store1.integrate_ops(op_grid(&arq, 10..20).into_iter());

        let extra_ops = [OpData::fake(-300i32 as u32, 18, 4), OpData::fake(12, 12, 4)];
        let mut store2 = store1.clone();
        store2.integrate_ops(extra_ops.clone().into_iter());

        let coords_a = RegionCoordSetXtcs::new(
            topo.telescoping_times(Timestamp::from_micros(20)),
            ArqSet::single(arq.clone()),
        );
        let coords_b = RegionCoordSetXtcs::new(
            topo.telescoping_times(Timestamp::from_micros(21)),
            ArqSet::single(arq.clone()),
        );

        let rset_a = RegionSetXtcs::new(&topo, &store1, coords_a);
        let rset_b = RegionSetXtcs::new(&topo, &store2, coords_b);
        assert_ne!(rset_a.data, rset_b.data);

        let diff = rset_a.clone().diff(rset_b.clone(), &topo).unwrap();
        assert_eq!(diff.len(), 2);

        assert!(diff[0].coords.contains(&extra_ops[0].coords(&topo)));
        assert!(diff[1].coords.contains(&extra_ops[1].coords(&topo)));

        // Adding the region data from each extra op to the region data of the
        // diff which was missing those ops should be the same as the query
        // of the store which contains the extra ops over the same region
        // TODO: proptest this
        assert_eq!(
            diff[0].data + extra_ops[0].region_data(),
            store2.query_region_coords(&diff[0].coords)
        );
        assert_eq!(
            diff[1].data + extra_ops[1].region_data(),
            store2.query_region_coords(&diff[1].coords)
        );
    }
}
