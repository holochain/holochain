//! Types representing a set of Arqs all of the same "power".

use kitsune_p2p_dht_arc::DhtArcSet;

use crate::{
    arq::ArqBounds,
    spacetime::{SpaceOffset, Topology},
    ArqStrat,
};

use super::{power_and_count_from_length, ArqBoundsSans, ArqImpl, ArqLoc, ArqStart, Topo};

/// A collection of ArqBounds.
/// All bounds are guaranteed to be quantized to the same power
/// (the lowest common power).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    derive_more::Deref,
    derive_more::DerefMut,
    derive_more::IntoIterator,
    derive_more::Index,
    derive_more::IndexMut,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct NonEmptyArqSet<T = Topo> {
    #[into_iterator]
    #[deref]
    #[deref_mut]
    #[index]
    #[index_mut]
    // #[serde(bound(deserialize = "S: serde::de::DeserializeOwned"))]
    pub(crate) arqs: Vec<ArqBoundsSans>,
    power: u8,
    topo: T,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, derive_more::From)]
pub enum ArqSet<T = Topo> {
    Empty([ArqBoundsSans; 0], T),
    NonEmpty(NonEmptyArqSet<T>),
}

/// ArqSet with Topology not set (used for serialization)
pub type ArqSetSans = ArqSet<()>;

impl<T: Clone + PartialEq> NonEmptyArqSet<T> {
    /// Normalize all arqs to be of the same power (use the minimum power)
    pub fn new(arqs: Vec<ArqBounds<T>>) -> Self {
        assert!(
            arqs.iter().all(|a| a.topo == arqs[0].topo),
            "all arqs in a set must have the same topology"
        );
        let topo = arqs[0].topo.clone();
        Self::new_sans(topo, arqs.into_iter().map(|a| a.sans()).collect())
    }

    /// Normalize all arqs to be of the same power (use the minimum power)
    pub fn new_sans(topo: T, arqs: Vec<ArqBoundsSans>) -> Self {
        let pow = arqs
            .iter()
            .map(|a| a.power())
            .min()
            .expect("min exists for nonempty set");

        Self {
            arqs: arqs
                .into_iter()
                .map(|a| a.requantize(pow).unwrap())
                .collect(),
            power: pow,
            topo,
        }
    }
}

impl NonEmptyArqSet<Topo> {
    /// Requantize each arq in the set.
    pub fn requantize(&self, power: u8) -> Option<Self> {
        self.arqs
            .iter()
            .map(|a| a.topo(self.topo.clone()).requantize(power))
            .collect::<Option<Vec<_>>>()
            .map(|arqs| Self::new(arqs))
    }

    /// Convert to a set of "continuous" arcs
    pub fn to_dht_arc_set(&self) -> DhtArcSet {
        DhtArcSet::from(
            self.arqs
                .iter()
                .map(|a| a.topo(self.topo.clone()).to_dht_arc_range())
                .collect::<Vec<_>>(),
        )
    }

    pub fn topo(&self) -> Topo {
        self.topo.clone()
    }
}

impl<T: Clone + PartialEq> ArqSet<T> {
    /// Get a reference to the arq set's arqs.
    pub fn arqs_sans(&self) -> &[ArqBoundsSans] {
        match self {
            Self::Empty(a, _) => a.as_slice(),
            Self::NonEmpty(s) => s.arqs.as_slice(),
        }
    }

    /// Empty set
    pub fn empty(topo: T) -> Self {
        Self::new::<()>(topo, vec![])
    }

    /// Normalize all arqs to be of the same power (use the minimum power)
    pub fn new<TT: Clone>(topo: T, arqs: Vec<ArqBounds<TT>>) -> Self {
        if arqs.is_empty() {
            Self::Empty([], topo)
        } else {
            Self::NonEmpty(NonEmptyArqSet::new_sans(
                topo,
                arqs.into_iter().map(|a| a.sans()).collect(),
            ))
        }
    }

    /// Map over the nonempty variant
    pub fn nonempty<R>(&self, f: impl FnOnce(&NonEmptyArqSet<T>) -> R) -> Option<R> {
        match self {
            Self::Empty(_, t) => None,
            Self::NonEmpty(set) => Some(f(set)),
        }
    }

    /// Erase topology info
    pub fn sans(self) -> ArqSet<()> {
        match self {
            Self::Empty(v, t) => ArqSet::Empty(v, ()),
            Self::NonEmpty(s) => ArqSet::NonEmpty(NonEmptyArqSet {
                arqs: s.arqs,
                power: s.power,
                topo: (),
            }),
        }
    }
}

impl ArqSet<()> {
    pub fn topo(self, topo: Topo) -> ArqSet<Topo> {
        match self {
            Self::Empty(v, ()) => ArqSet::Empty(v, topo),
            Self::NonEmpty(s) => ArqSet::NonEmpty(NonEmptyArqSet {
                arqs: s.arqs,
                power: s.power,
                topo,
            }),
        }
    }
}

impl ArqSet {
    /// Singleton set
    pub fn single(arq: ArqBounds) -> Self {
        Self::new(arq.topo(), vec![arq.sans()])
    }

    /// Get a reference to the arq set's arqs.
    pub fn arqs(&self) -> Vec<ArqBounds> {
        match self {
            Self::Empty(_, t) => vec![],
            Self::NonEmpty(s) => s.iter().map(|a| a.topo(s.topo.clone())).collect(),
        }
    }

    /// Get a reference to the arq set's power.
    pub fn power(&self) -> Option<u8> {
        self.nonempty(|s| s.power)
    }

    /// Convert to a set of "continuous" arcs
    pub fn to_dht_arc_set(&self) -> DhtArcSet {
        self.nonempty(|s| s.to_dht_arc_set())
            .unwrap_or_else(|| DhtArcSet::new_empty())
    }

    /// Requantize each arq in the set.
    pub fn requantize(&self, power: u8) -> Option<Self> {
        self.nonempty(|s| {
            s.arqs
                .iter()
                .map(|a| a.topo(s.topo.clone()).requantize(power).map(|b| b.sans()))
                .collect::<Option<Vec<_>>>()
                .map(|arqs| Self::new(s.topo(), arqs))
        })
        .unwrap_or_else(|| Some(self.clone()))
    }

    pub fn topo(&self) -> Topo {
        match self {
            ArqSet::Empty(_, topo) => topo.clone(),
            ArqSet::NonEmpty(s) => s.topo.clone(),
        }
    }

    /// Intersection of all arqs contained within
    pub fn intersection(&self, other: &Self) -> ArqSet {
        assert!(self.topo() == other.topo(), "topologies must match");
        match (self, other) {
            (ArqSet::Empty(_, t), ArqSet::Empty(_, _)) => Self::empty(t.clone()),
            (ArqSet::Empty(_, t), ArqSet::NonEmpty(_)) => Self::empty(t.clone()),
            (ArqSet::NonEmpty(_), ArqSet::Empty(_, t)) => Self::empty(t.clone()),
            (ArqSet::NonEmpty(a), ArqSet::NonEmpty(b)) => {
                let topo = a.topo.clone();
                let power = a.power.min(b.power);
                let a1 = a.requantize(power).unwrap().to_dht_arc_set();
                let a2 = b.requantize(power).unwrap().to_dht_arc_set();
                ArqSet::NonEmpty(NonEmptyArqSet {
                    arqs: DhtArcSet::intersection(&a1, &a2)
                        .intervals()
                        .into_iter()
                        .map(|interval| {
                            ArqBounds::from_interval(topo.clone(), power, interval)
                                .expect("cannot fail")
                                .sans()
                        })
                        .collect(),
                    power,
                    topo,
                })
            }
        }
    }

    /// View ascii for all arq bounds
    pub fn print_arqs(&self, len: usize) {
        match self {
            Self::NonEmpty(set) => {
                println!("{} arqs, power: {:?}", set.arqs.len(), set.power);
                for (i, arq) in set.arqs.iter().enumerate() {
                    println!(
                        "{:>3}: |{}| {}/{} @ {:?}",
                        i,
                        arq.topo(set.topo.clone()).to_ascii(len),
                        arq.power(),
                        arq.count(),
                        arq.start
                    );
                }
            }
            ArqSet::Empty(_, t) => println!("[empty ArqSet]"),
        }
    }

    /// Convert back from a continuous arc set to a quantized one.
    /// If any information is lost (the match is not exact), return None.
    pub fn from_dht_arc_set(topo: Topo, strat: &ArqStrat, dht_arc_set: &DhtArcSet) -> Option<Self> {
        let max_chunks = strat.max_chunks();
        Some(Self::new(
            topo.clone(),
            dht_arc_set
                .intervals()
                .into_iter()
                .map(|i| {
                    let len = i.length();
                    let (pow, _) = power_and_count_from_length(&topo.space, len, max_chunks);
                    ArqBounds::from_interval(topo.clone(), pow, i).map(ArqBounds::sans)
                })
                .collect::<Option<Vec<_>>>()?,
        ))
    }

    /// Convert back from a continuous arc set to a quantized one.
    /// If the match is not exact, return the nearest possible quantized arc.
    pub fn from_dht_arc_set_rounded(
        topo: Topo,
        strat: &ArqStrat,
        dht_arc_set: &DhtArcSet,
    ) -> (Self, bool) {
        let max_chunks = strat.max_chunks();
        let mut rounded = false;
        let arqs = dht_arc_set
            .intervals()
            .into_iter()
            .map(|i| {
                let len = i.length();
                let (pow, _) = power_and_count_from_length(&topo.space, len, max_chunks);
                let (a, r) = ArqBounds::from_interval_rounded(topo.clone(), pow, i);
                if r {
                    rounded = true;
                }
                a.sans()
            })
            .collect::<Vec<_>>();
        (Self::new(topo, arqs), rounded)
    }
}

/// Print ascii for arq bounds
pub fn print_arq<S: ArqStart>(arq: &ArqImpl<S, Topo>, len: usize) {
    println!("|{}| {} *2^{}", arq.to_ascii(len), arq.count(), arq.power());
}

/// Print a collection of arqs
pub fn print_arqs<S: ArqStart>(arqs: &[ArqImpl<S, Topo>], len: usize) {
    for (i, arq) in arqs.iter().enumerate() {
        println!(
            "|{}| {}:\t{} +{} *2^{}",
            arq.to_ascii(len),
            i,
            *arq.start.to_offset(&arq.topo, arq.power()),
            arq.count(),
            arq.power()
        );
    }
}

#[cfg(test)]
mod tests {

    use crate::prelude::{pow2, ArqLocTopo};

    use super::*;

    #[test]
    fn intersect_arqs() {
        holochain_trace::test_run().ok();
        let topo = Topology::unit_zero();
        let a = ArqLocTopo::new(topo.clone(), 27, 536870912u32.into(), 11.into()).to_bounds();
        let b = ArqLocTopo::new(topo.clone(), 27, 805306368u32.into(), 11.into()).to_bounds();
        dbg!(a.offset());

        let a = ArqSet::single(a);
        let b = ArqSet::single(b);
        let c = a.intersection(&b);
        print_arqs(&a.arqs(), 64);
        print_arqs(&b.arqs(), 64);
        print_arqs(&c.arqs(), 64);
    }

    #[test]
    fn intersect_arqs_multi() {
        holochain_trace::test_run().ok();
        let topo = Topology::unit_zero();

        let pow = 26;
        let sa1 = (u32::MAX - 4 * pow2(pow) + 1).into();
        let sa2 = (13 * pow2(pow - 1)).into();
        let sb1 = 0u32.into();
        let sb2 = (20 * pow2(pow - 1)).into();

        let a = ArqSet::new(
            topo.clone(),
            vec![
                ArqLocTopo::new(topo.clone(), pow, sa1, 8.into()).to_bounds(),
                ArqLocTopo::new(topo.clone(), pow - 1, sa2, 8.into()).to_bounds(),
            ],
        );
        let b = ArqSet::new(
            topo.clone(),
            vec![
                ArqLocTopo::new(topo.clone(), pow, sb1, 8.into()).to_bounds(),
                ArqLocTopo::new(topo.clone(), pow - 1, sb2, 8.into()).to_bounds(),
            ],
        );

        let c = a.intersection(&b);
        print_arqs(&a.arqs(), 64);
        println!();
        print_arqs(&b.arqs(), 64);
        println!();
        // the last arq of c doesn't show up in the ascii representation, but
        // it is there.
        print_arqs(&c.arqs(), 64);

        let arqs = c.arqs_sans();
        assert_eq!(arqs.len(), 3);
        assert_eq!(arqs[0].start, 0.into());
        assert_eq!(arqs[1].start, 13.into());
        assert_eq!(arqs[2].start, 20.into());
    }

    #[test]
    fn normalize_arqs() {
        let s = ArqSet::new(
            (),
            vec![
                ArqBoundsSans::new(10, 0.into(), SpaceOffset(10)),
                ArqBoundsSans::new(8, 0.into(), SpaceOffset(40)),
                ArqBoundsSans::new(12, 0.into(), SpaceOffset(3)),
            ],
        );

        assert_eq!(
            s.arqs_sans(),
            &[
                ArqBoundsSans::new(8, 0.into(), SpaceOffset(4 * 10)),
                ArqBoundsSans::new(8, 0.into(), SpaceOffset(40)),
                ArqBoundsSans::new(8, 0.into(), SpaceOffset(3 * 16)),
            ]
        );
    }

    proptest::proptest! {
        #[test]
        fn rounded_arcset_intersections(p1 in 0u8..15, s1: u32, c1 in 8u32..64, p2 in 0u8..15, s2: u32, c2 in 8u32..64) {
            use crate::Loc;

            let topo = Topology::standard_epoch_full();
            let arq1 = ArqLocTopo::new(topo.clone(), p1, Loc::from(s1), c1.into());
            let arq2 = ArqLocTopo::new(topo.clone(), p2, Loc::from(s2), c2.into());
            let arcset1: DhtArcSet = arq1.to_bounds().to_dht_arc_range().into();
            let arcset2: DhtArcSet = arq2.to_bounds().to_dht_arc_range().into();
            let common = arcset1.intersection(&arcset2);
            let ii = common.intervals();
            for i in ii {
                let p = p1.min(p2);
                dbg!(&p, &i);
                let (_, rounded) = ArqBounds::from_interval_rounded(topo.clone(), p, i);
                assert!(!rounded);
            }
        }
    }
}
