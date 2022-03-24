use once_cell::sync::OnceCell;

use crate::{
    arq::*,
    error::{GossipError, GossipResult},
    op::OpRegion,
    persistence::AccessOpStore,
    quantum::*,
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
    pub fn region_coords_flat(&self) -> impl Iterator<Item = ((u32, u32), RegionCoords)> + '_ {
        self.region_coords_nested().flatten()
    }

    /// Iterate over the coords in the same structure in which they are stored:
    /// An outer Vec corresponding to space segments,
    /// and inner Vecs corresponding to time segments.
    pub fn region_coords_nested(
        &self,
    ) -> impl Iterator<Item = impl Iterator<Item = ((u32, u32), RegionCoords)>> + '_ {
        self.arq_set.arqs().iter().flat_map(move |arq| {
            arq.segments().enumerate().map(move |(ix, x)| {
                self.times
                    .segments()
                    .into_iter()
                    .enumerate()
                    .map(move |(it, t)| ((ix as u32, it as u32), RegionCoords::new(x, t)))
            })
        })
    }

    /// Generate data for each coord in the set, creating the corresponding [`RegionSetLtcs`].
    pub fn into_region_set<D, E, F>(self, mut f: F) -> Result<RegionSetLtcs<D>, E>
    where
        D: RegionDataConstraints,
        F: FnMut(((u32, u32), RegionCoords)) -> Result<D, E>,
    {
        let data = self
            .region_coords_nested()
            .map(move |column| column.map(&mut f).collect::<Result<Vec<D>, E>>())
            .collect::<Result<Vec<Vec<D>>, E>>()?;
        Ok(RegionSetLtcs::from_data(self, data))
    }

    /// An empty set of coords
    pub fn empty() -> Self {
        Self {
            times: TelescopingTimes::empty(),
            arq_set: ArqBoundsSet::empty(),
        }
    }
}

/// Implementation for the compact LTCS region set format which gets sent over the wire.
/// The coordinates for the regions are specified by a few values.
/// The data to match the coordinates are specified in a 2D vector which must
/// correspond to the generated coordinates.
#[derive(Debug, serde::Serialize, serde::Deserialize, Derivative)]
#[derivative(PartialEq, Eq)]
#[cfg_attr(feature = "test_utils", derive(Clone))]
pub struct RegionSetLtcs<D: RegionDataConstraints = RegionData> {
    /// The generator for the coordinates
    pub coords: RegionCoordSetLtcs,

    /// the actual coordinates as generated
    #[derivative(PartialEq = "ignore")]
    #[serde(skip)]
    pub(crate) _region_coords: OnceCell<Vec<RegionCoords>>,

    /// The outer vec corresponds to the spatial segments;
    /// the inner vecs are the time segments.
    #[serde(bound(deserialize = "D: serde::de::DeserializeOwned"))]
    pub data: Vec<Vec<D>>,
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
    pub fn from_data(coords: RegionCoordSetLtcs, data: Vec<Vec<D>>) -> Self {
        Self {
            coords,
            data,
            _region_coords: OnceCell::new(),
        }
    }

    /// Query the specified OpStore for each coord in the set, constructing
    /// the full RegionSet.
    /// TODO: can probably implement in terms of RegionCoords::into_region_set()
    pub fn from_store<O: OpRegion<D>, S: AccessOpStore<O, D>>(
        store: &S,
        coords: RegionCoordSetLtcs,
    ) -> Self {
        let data = coords
            .region_coords_nested()
            .map(|columns| {
                columns
                    .map(|(_, coords)| store.query_region_data(&coords))
                    .collect()
            })
            .collect();
        Self {
            coords,
            data,
            _region_coords: OnceCell::new(),
        }
    }

    /// The total number of regions represented in this region set
    pub fn count(&self) -> usize {
        if self.data.is_empty() {
            0
        } else {
            self.data.len() * self.data[0].len()
        }
    }

    /// Iterate over each region in the set
    pub fn regions(&self) -> impl Iterator<Item = Region<D>> + '_ {
        self.coords
            .region_coords_flat()
            .map(|((ix, it), coords)| Region::new(coords, self.data[ix as usize][it as usize]))
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
            TelescopingTimes::rectify((&self.coords.times, da), (&other.coords.times, db));
            len = da.len();
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
}

#[cfg(feature = "test_utils")]
impl RegionSetLtcs {
    /// Return only the regions which have ops in them. Useful for testing
    /// sparse scenarios.
    pub fn nonzero_regions(
        &self,
    ) -> impl '_ + Iterator<Item = ((u32, u32), RegionCoords, RegionData)> {
        self.coords.region_coords_flat().filter_map(|((i, j), c)| {
            let d = &self.data[i as usize][j as usize];
            (d.count > 0).then(|| ((i, j), c, *d))
        })
    }
}
