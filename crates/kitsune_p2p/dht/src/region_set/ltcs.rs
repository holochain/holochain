use once_cell::sync::OnceCell;

use crate::{
    arq::*,
    error::{GossipError, GossipResult},
    op::OpRegion,
    spacetime::*,
};
use derivative::Derivative;

use super::{Region, RegionCoords, RegionData, RegionDataConstraints};

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
    pub fn into_region_set<D, F, E>(self, mut f: F) -> Result<RegionSetLtcs<D>, E>
    where
        D: RegionDataConstraints,
        F: FnMut(((usize, usize, usize), RegionCoords)) -> Result<D, E>,
    {
        let data = self
            .region_coords_nested()
            .map(|arqdata| {
                arqdata
                    .map(|column| column.map(&mut f).collect::<Result<Vec<D>, E>>())
                    .collect::<Result<Vec<Vec<D>>, E>>()
            })
            .collect::<Result<Vec<Vec<Vec<D>>>, E>>()?;
        Ok(RegionSetLtcs::from_data(self, data))
    }

    /// Generate data for each coord in the set, creating the corresponding [`RegionSetLtcs`],
    /// using a mapping function which cannot fail.
    pub fn into_region_set_infallible<D, F>(self, mut f: F) -> RegionSetLtcs<D>
    where
        D: RegionDataConstraints,
        F: FnMut(((usize, usize, usize), RegionCoords)) -> D,
    {
        self.into_region_set(|c| Result::<D, std::convert::Infallible>::Ok(f(c)))
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
#[derive(serde::Serialize, serde::Deserialize, Derivative)]
#[derivative(PartialEq, Eq)]
#[cfg_attr(feature = "test_utils", derive(Clone))]
pub struct RegionSetLtcs<D: RegionDataConstraints = RegionData> {
    /// The generator for the coordinates
    pub coords: RegionCoordSetLtcs,

    /// the actual coordinates as generated
    #[derivative(PartialEq = "ignore")]
    #[serde(skip)]
    pub(crate) _region_coords: OnceCell<Vec<RegionCoords>>,

    /// The outermost vec corresponds to arqs in the ArqSet;
    /// The middle vecs correspond to the spatial segments per arq;
    /// the innermost vecs are the time segments per arq.
    #[serde(bound(deserialize = "D: serde::de::DeserializeOwned"))]
    pub data: Vec<Vec<Vec<D>>>,
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
            _region_coords: OnceCell::new(),
        }
    }

    /// Construct the region set from existing data.
    /// The data must match the coords!
    pub fn from_data(coords: RegionCoordSetLtcs, data: Vec<Vec<Vec<D>>>) -> Self {
        Self {
            coords,
            data,
            _region_coords: OnceCell::new(),
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
                Region::new(coords, self.data[ia][ix as usize][it as usize].clone())
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
    pub fn diff(mut self, mut other: Self) -> GossipResult<Vec<Region<D>>> {
        self.rectify(&mut other)?;

        let regions = self
            .regions()
            .into_iter()
            .zip(other.regions().into_iter())
            .filter_map(|(a, b)| (a.data != b.data).then(|| a))
            .collect();

        Ok(regions)
    }

    /// Return only the regions which have ops in them. Useful for testing
    /// sparse scenarios.
    pub fn nonzero_regions(
        &self,
    ) -> impl '_ + Iterator<Item = ((usize, usize, usize), RegionCoords, D)> {
        self.coords
            .region_coords_flat()
            .filter_map(|((a, x, y), c)| {
                let d = &self
                    .data
                    .get(a)
                    .and_then(|d| d.get(x))
                    .and_then(|d| d.get(y));
                d.filter(|d| d.count() > 0)
                    .map(|d| ((a, x, y), c, d.clone()))
            })
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
        coords.into_region_set_infallible(|(_, coords)| store.query_region_data(&coords))
    }
}
