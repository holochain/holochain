use std::collections::HashSet;

use crate::{
    arq::*,
    error::{GossipError, GossipResult},
    op::OpRegion,
    region::RegionCell,
    spacetime::*,
};
use derivative::Derivative;

use super::{Region, RegionCoords, RegionData, RegionDataConstraints};

/// The nested set of data for a RegionSet
pub type RegionDataGrid<D> = Vec<Vec<Vec<RegionCell<D>>>>;

/// A compact representation of a set of [`RegionCoords`].
/// The [`TelescopingTimes`] generates all relevant [`TimeSegment`]s, and the
/// [`SpaceSegment`]s are implied by the [`ArqBoundsSet`].
///
/// LTCS stands for Logarithmic Time, Constant Space.
#[derive(Debug, PartialEq, Eq, derive_more::Constructor, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "test_utils", derive(Clone))]
pub struct RegionCoordSetLtcs {
    pub(super) times: TelescopingTimes,
    pub(super) arq_set: ArqBoundsSet,
}

impl RegionCoordSetLtcs {
    /// Generate the LTCS region coords given the generating parameters.
    /// Each RegionCoords is paired with the relative spacetime coords, which
    /// can be used to pair the generated coords with stored data.
    #[cfg_attr(not(feature = "test_utils"), deprecated = "use into_region_set")]
    pub(crate) fn region_coords_flat(
        &self,
    ) -> impl Iterator<Item = ((usize, usize, usize), RegionCoords)> + '_ {
        self.region_coords_nested().flatten().flatten()
    }

    /// Iterate over the coords in the same structure in which they are stored:
    /// An outermost Vec corresponding to the arqs,
    /// middle Vecs corresponding to space segments per arq,
    /// and inner Vecs corresponding to time segments per arq.
    #[cfg_attr(not(feature = "test_utils"), deprecated = "use into_region_set")]
    pub(crate) fn region_coords_nested(
        &self,
    ) -> impl Iterator<
        Item = impl Iterator<Item = impl Iterator<Item = ((usize, usize, usize), RegionCoords)>> + '_,
    > + '_ {
        let arqs = self.arq_set.arqs();
        arqs.iter().enumerate().map(move |(ia, arq)| {
            arq.segments().enumerate().map(move |(ix, x)| {
                self.times
                    .segments()
                    .into_iter()
                    .enumerate()
                    .map(move |(it, t)| ((ia, ix, it), RegionCoords::new(x, t)))
            })
        })
    }

    /// Generate data for each coord in the set, creating the corresponding [`RegionSetLtcs`].
    pub fn into_region_set<D, F, E>(
        self,
        locked_regions: HashSet<RegionCoords>,
        mut f: F,
    ) -> Result<RegionSetLtcs<D>, E>
    where
        D: RegionDataConstraints,
        F: FnMut(((usize, usize, usize), RegionCoords)) -> Result<D, E>,
    {
        let data = self
            .region_coords_nested()
            .map(|arqdata| {
                arqdata
                    .map(|column| {
                        column
                            .map(|t| {
                                Ok(if locked_regions.contains(&t.1) {
                                    RegionCell::Locked
                                } else {
                                    RegionCell::Data(f(t)?)
                                })
                            })
                            .collect::<Result<Vec<RegionCell<D>>, E>>()
                    })
                    .collect::<Result<Vec<Vec<_>>, E>>()
            })
            .collect::<Result<Vec<Vec<Vec<_>>>, E>>()?;
        let set = RegionSetLtcs { coords: self, data };
        Ok(set)
    }

    /// Generate data for each coord in the set, creating the corresponding [`RegionSetLtcs`],
    /// using a mapping function which cannot fail.
    #[cfg(feature = "test_utils")]
    pub fn into_region_set_infallible_unlocked<D, F>(self, mut f: F) -> RegionSetLtcs<D>
    where
        D: RegionDataConstraints,
        F: FnMut(((usize, usize, usize), RegionCoords)) -> D,
    {
        self.into_region_set(Default::default(), |c| {
            Result::<D, std::convert::Infallible>::Ok(f(c))
        })
        .unwrap()
    }

    /// An empty set of coords
    pub fn empty() -> Self {
        Self {
            times: TelescopingTimes::empty(),
            arq_set: ArqBoundsSet::empty(),
        }
    }

    /// Return the number of chunks in the arq set
    pub fn num_space_chunks(&self) -> usize {
        self.arq_set.arqs().len()
    }

    /// The total number of coords represented here.
    pub fn count(&self) -> usize {
        let nt = self.times.segments().len();
        self.arq_set.arqs().iter().map(|a| a.count()).sum::<u32>() as usize * nt
    }
}

/// Implementation for the compact LTCS region set format which gets sent over the wire.
/// The coordinates for the regions are specified by a few values.
/// The data to match the coordinates are specified in a 2D vector which must
/// correspond to the generated coordinates.
#[derive(PartialEq, Eq, serde::Serialize, serde::Deserialize, Derivative)]
#[cfg_attr(feature = "test_utils", derive(Clone))]
pub struct RegionSetLtcs<D: RegionDataConstraints = RegionData> {
    /// The generator for the coordinates
    pub coords: RegionCoordSetLtcs,

    /// The outermost vec corresponds to arqs in the ArqSet;
    /// The middle vecs correspond to the spatial segments per arq;
    /// the innermost vecs are the time segments per arq.
    #[serde(bound(deserialize = "D: serde::de::DeserializeOwned"))]
    data: RegionDataGrid<D>,
}

impl<D: RegionDataConstraints> std::fmt::Debug for RegionSetLtcs<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegionSetLtcs")
            .field(
                "nonzero_regions",
                &self.nonzero_regions().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl<D: RegionDataConstraints> RegionSetLtcs<D> {
    /// An empty LTCS region set
    pub fn empty() -> Self {
        Self {
            coords: RegionCoordSetLtcs::empty(),
            data: vec![],
        }
    }

    /// The total number of regions represented in this region set
    pub fn count(&self) -> usize {
        self.data
            .iter()
            .map(|d| {
                if d.is_empty() {
                    0
                } else {
                    // All inner lengths must be equal
                    debug_assert!(d.iter().all(|i| i.len() == d[0].len()));
                    d.len() * d[0].len()
                }
            })
            .sum()
    }

    /// Iterate over each region in the set
    pub fn regions(&self) -> impl Iterator<Item = Region<D>> + '_ {
        self.coords
            .region_coords_flat()
            .map(|((ia, ix, it), coords)| {
                let data = self.data[ia][ix as usize][it as usize].clone();
                Region::new(coords, data)
            })
    }

    /// Reshape the two region sets so that both match, omitting or merging
    /// regions as needed
    pub fn rectify(&mut self, other: &mut Self) -> GossipResult<()> {
        if self.coords.arq_set != other.coords.arq_set {
            return Err(GossipError::ArqSetMismatchForDiff);
        }
        if self.coords.times > other.coords.times {
            std::mem::swap(self, other);
        }
        let mut len = 0;
        for (da, db) in self.data.iter_mut().zip(other.data.iter_mut()) {
            for (dda, ddb) in da.iter_mut().zip(db.iter_mut()) {
                TelescopingTimes::rectify((&self.coords.times, dda), (&other.coords.times, ddb));
                len = dda.len();
            }
        }
        let times = other.coords.times.limit(len as u32);
        self.coords.times = times;
        other.coords.times = times;
        Ok(())
    }

    /// Given two region sets, return only the ones which are different between
    /// the two
    pub fn diff(mut self, mut other: Self) -> GossipResult<RegionDiffs<D>> {
        self.rectify(&mut other)?;

        let (ours, theirs) = self
            .regions()
            .into_iter()
            .zip(other.regions().into_iter())
            // Any regions which are declared locked, or which match perfectly, are excluded
            .filter(|(a, b)| !(a.data.is_locked() || b.data.is_locked() || a.data == b.data))
            .unzip();

        Ok(RegionDiffs { ours, theirs })
    }

    /// Return only the regions which have ops in them. Useful for testing
    /// sparse scenarios.
    pub fn nonzero_regions(
        &self,
    ) -> impl '_ + Iterator<Item = ((usize, usize, usize), RegionCoords, RegionCell<D>)> {
        self.coords
            .region_coords_flat()
            .filter_map(|((a, x, y), c)| {
                let d = &self
                    .data
                    .get(a)
                    .and_then(|d| d.get(x))
                    .and_then(|d| d.get(y));
                d.filter(|d| d.as_option().map(|d| d.count() > 0).unwrap_or(false))
                    .map(|d| ((a, x, y), c, d.clone()))
            })
    }

    /// Accessor
    pub fn data(&self) -> &[Vec<Vec<RegionCell<D>>>] {
        self.data.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// The diff of two region sets, from the perspective of both parties
///
/// Both sets of regions will have the same coordinates: what's different is the data
/// in each region. This *could* be expressed a single set of RegionCoords and two
/// sets of data to go with each coord, but it's not worth the trouble.
pub struct RegionDiffs<D> {
    /// The regions that "we" (this node) hold
    pub ours: Vec<Region<D>>,
    /// The regions that they (our gossip partner) hold
    pub theirs: Vec<Region<D>>,
}

impl<D: RegionDataConstraints> RegionDiffs<D> {
    /// Pick regions from both sets up until the max_size is reached, skipping locked regions,
    /// and discard all others
    pub fn round_limited(self, max_size: u32) -> (Self, bool) {
        let len_ours = self.ours.len();
        let len_theirs = self.theirs.len();
        let ours = Self::round_limited_1(self.ours, max_size);
        let theirs = Self::round_limited_1(self.theirs, max_size);
        let diffs = Self { ours, theirs };
        let limited = diffs.ours.len() < len_ours || diffs.theirs.len() < len_theirs;
        (diffs, limited)
    }

    fn round_limited_1(regions: Vec<Region<D>>, max_size: u32) -> Vec<Region<D>> {
        use itertools::FoldWhile::{Continue, Done};
        use itertools::Itertools;

        let (limited, _) = regions
            .into_iter()
            .enumerate()
            .fold_while((vec![], 0), |(mut v, total), (i, r)| match &r.data {
                RegionCell::Data(data) => {
                    let size = data.size();
                    if i == 0 || total + size <= max_size {
                        v.push(r.clone());
                        Continue((v, total + size))
                    } else {
                        Done((v, total))
                    }
                }
                RegionCell::Locked => Continue((v, total)),
            })
            .into_inner();
        limited
    }
}

impl<D> Default for RegionDiffs<D> {
    fn default() -> Self {
        Self {
            ours: vec![],
            theirs: vec![],
        }
    }
}

#[cfg(feature = "test_utils")]
impl<D: RegionDataConstraints> RegionSetLtcs<D> {
    /// Query the specified OpStore for each coord in the set, constructing
    /// the full RegionSet. Purely for convenience.
    pub fn from_store<O: OpRegion<D>, S: crate::persistence::AccessOpStore<O, D>>(
        store: &S,
        coords: RegionCoordSetLtcs,
    ) -> Self {
        coords.into_region_set_infallible_unlocked(|(_, coords)| store.query_region_data(&coords))
    }
}

#[cfg(test)]
mod tests {

    use std::convert::Infallible;

    use num_traits::Zero;

    use super::*;

    #[test]
    fn test_round_limiting() {
        let num_coords = 20;
        let coords = RegionCoordSetLtcs::new(
            TelescopingTimes::new(1.into()),
            ArqBoundsSet::new(vec![ArqBounds::new(8, 0.into(), num_coords.into())]),
        );
        assert_eq!(coords.count(), num_coords as usize);

        let locked: HashSet<_> = coords
            .region_coords_flat()
            .map(|(_, c)| c)
            .take(12)
            .collect();

        let diffs = {
            let a = coords
                .clone()
                .into_region_set_infallible_unlocked(|_| RegionData {
                    count: 1,
                    size: 100,
                    hash: Zero::zero(),
                });
            let b = coords
                .clone()
                .into_region_set_infallible_unlocked(|_| RegionData {
                    count: 1,
                    size: 200,
                    hash: Zero::zero(),
                });
            a.diff(b).unwrap()
        };

        let diffs_locked = {
            let al = coords
                .clone()
                .into_region_set(locked.clone(), |_| {
                    Result::<_, Infallible>::Ok(RegionData {
                        count: 1,
                        size: 100,
                        hash: Zero::zero(),
                    })
                })
                .unwrap();
            let bl = coords
                .clone()
                .into_region_set(locked.clone(), |_| {
                    Result::<_, Infallible>::Ok(RegionData {
                        count: 1,
                        size: 200,
                        hash: Zero::zero(),
                    })
                })
                .unwrap();

            al.diff(bl).unwrap()
        };
        {
            let (diffs_10k, lim) = diffs.clone().round_limited(10000);
            assert!(!lim);
            assert_eq!(diffs, diffs_10k);
        }
        {
            let (diffs_1k, lim) = diffs.clone().round_limited(1000);
            assert!(lim);
            assert_eq!(diffs_1k.ours.len(), 10);
            assert_eq!(diffs_1k.theirs.len(), 5);
            assert_eq!(
                diffs_1k.ours.iter().map(|r| r.data.size()).sum::<u32>(),
                1000
            );
            assert_eq!(
                diffs_1k.theirs.iter().map(|r| r.data.size()).sum::<u32>(),
                1000
            );
        }
        {
            let (diffs_1k_locked, lim) = diffs_locked.clone().round_limited(1000);
            assert!(lim);
            assert_eq!(diffs_1k_locked.ours.len(), 8);
            assert_eq!(diffs_1k_locked.theirs.len(), 5);
            // - we're constrained by the lack of unlocked regions
            assert_eq!(
                diffs_1k_locked
                    .ours
                    .iter()
                    .map(|r| r.data.size())
                    .sum::<u32>(),
                800
            );
            // - we're still constrained by the size limit
            assert_eq!(
                diffs_1k_locked
                    .theirs
                    .iter()
                    .map(|r| r.data.size())
                    .sum::<u32>(),
                1000
            );
        }
        {
            let (diffs_5k, lim) = diffs.round_limited(5000);
            assert!(lim);
            assert_eq!(diffs_5k.ours.len(), 20);
            assert_eq!(diffs_5k.theirs.len(), 20);
            assert_eq!(
                diffs_5k.ours.iter().map(|r| r.data.size()).sum::<u32>(),
                2000
            );
            assert_eq!(
                diffs_5k.theirs.iter().map(|r| r.data.size()).sum::<u32>(),
                4000
            );
        }
    }
}
