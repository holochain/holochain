//! Types representing a set of Arqs all of the same "power".

use kitsune_p2p_dht_arc::DhtArcSet;

use crate::{
    arq::ArqBounds,
    spacetime::{SpaceOffset, Topology},
    ArqStrat,
};

use super::{power_and_count_from_length, Arq, ArqStart};

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
pub struct ArqSet<S: ArqStart = SpaceOffset> {
    #[into_iterator]
    #[deref]
    #[deref_mut]
    #[index]
    #[index_mut]
    #[serde(bound(deserialize = "S: serde::de::DeserializeOwned"))]
    pub(crate) arqs: Vec<Arq<S>>,
    power: u8,
}

impl<S: ArqStart> ArqSet<S> {
    /// Normalize all arqs to be of the same power (use the minimum power)
    pub fn new(arqs: Vec<Arq<S>>) -> Self {
        if let Some(pow) = arqs.iter().map(|a| a.power()).min() {
            Self {
                arqs: arqs
                    .into_iter()
                    .map(|a| a.requantize(pow).unwrap())
                    .collect(),
                power: pow,
            }
        } else {
            Self {
                arqs: vec![],
                power: 1,
            }
        }
    }

    /// Empty set
    pub fn empty() -> Self {
        Self::new(vec![])
    }

    /// Singleton set
    pub fn single(arq: Arq<S>) -> Self {
        Self::new(vec![arq])
    }

    /// Get a reference to the arq set's power.
    pub fn power(&self) -> u8 {
        self.power
    }

    /// Get a reference to the arq set's arqs.
    pub fn arqs(&self) -> &[Arq<S>] {
        self.arqs.as_ref()
    }

    /// Convert to a set of "continuous" arcs
    pub fn to_dht_arc_set(&self, topo: &Topology) -> DhtArcSet {
        DhtArcSet::from(
            self.arqs
                .iter()
                .map(|a| a.to_dht_arc_range(topo))
                .collect::<Vec<_>>(),
        )
    }

    /// Requantize each arq in the set.
    pub fn requantize(&self, power: u8) -> Option<Self> {
        self.arqs
            .iter()
            .map(|a| a.requantize(power))
            .collect::<Option<Vec<_>>>()
            .map(|arqs| Self { arqs, power })
    }

    /// Intersection of all arqs contained within
    pub fn intersection(&self, topo: &Topology, other: &Self) -> ArqSet<SpaceOffset> {
        let power = self.power.min(other.power());
        let a1 = self.requantize(power).unwrap().to_dht_arc_set(topo);
        let a2 = other.requantize(power).unwrap().to_dht_arc_set(topo);
        ArqSet {
            arqs: DhtArcSet::intersection(&a1, &a2)
                .intervals()
                .into_iter()
                .map(|interval| {
                    ArqBounds::from_interval(topo, power, interval).expect("cannot fail")
                })
                .collect(),
            power,
        }
    }

    /// View ascii for all arq bounds
    pub fn print_arqs(&self, topo: &Topology, len: usize) {
        println!("{} arqs, power: {}", self.arqs().len(), self.power());
        for (i, arq) in self.arqs().iter().enumerate() {
            println!(
                "{:>3}: |{}| {}/{} @ {:?}",
                i,
                arq.to_ascii(topo, len),
                arq.power(),
                arq.count(),
                arq.start
            );
        }
    }
}

impl ArqSet {
    /// Convert back from a continuous arc set to a quantized one.
    /// If any information is lost (the match is not exact), return None.
    pub fn from_dht_arc_set(
        topo: &Topology,
        strat: &ArqStrat,
        dht_arc_set: &DhtArcSet,
    ) -> Option<Self> {
        let max_chunks = strat.max_chunks();
        Some(Self::new(
            dht_arc_set
                .intervals()
                .into_iter()
                .map(|i| {
                    let len = i.length();
                    let (pow, _) = power_and_count_from_length(&topo.space, len, max_chunks);
                    ArqBounds::from_interval(topo, pow, i)
                })
                .collect::<Option<Vec<_>>>()?,
        ))
    }

    /// Convert back from a continuous arc set to a quantized one.
    /// If the match is not exact, return the nearest possible quantized arc.
    pub fn from_dht_arc_set_rounded(
        topo: &Topology,
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
                let (a, r) = ArqBounds::from_interval_rounded(topo, pow, i);
                if r {
                    rounded = true;
                }
                a
            })
            .collect::<Vec<_>>();
        (Self::new(arqs), rounded)
    }
}

/// Print ascii for arq bounds
pub fn print_arq<S: ArqStart>(topo: &Topology, arq: &Arq<S>, len: usize) {
    println!(
        "|{}| {} *2^{}",
        arq.to_ascii(topo, len),
        arq.count(),
        arq.power()
    );
}

/// Print a collection of arqs
pub fn print_arqs<S: ArqStart>(topo: &Topology, arqs: &[Arq<S>], len: usize) {
    for (i, arq) in arqs.iter().enumerate() {
        println!(
            "|{}| {}:\t{} +{} *2^{}",
            arq.to_ascii(topo, len),
            i,
            *arq.start.to_offset(topo, arq.power()),
            arq.count(),
            arq.power()
        );
    }
}

#[cfg(test)]
mod tests {

    use crate::prelude::pow2;

    use super::*;

    #[test]
    fn intersect_arqs() {
        holochain_trace::test_run().ok();
        let topo = Topology::unit_zero();
        let a = Arq::new(27, 536870912u32.into(), 11.into());
        let b = Arq::new(27, 805306368u32.into(), 11.into());
        dbg!(a.to_bounds(&topo).offset());

        let a = ArqSet::single(a);
        let b = ArqSet::single(b);
        let c = a.intersection(&topo, &b);
        print_arqs(&topo, &a, 64);
        print_arqs(&topo, &b, 64);
        print_arqs(&topo, &c, 64);
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

        let a = ArqSet::new(vec![
            Arq::new(pow, sa1, 8.into()).to_bounds(&topo),
            Arq::new(pow - 1, sa2, 8.into()).to_bounds(&topo),
        ]);
        let b = ArqSet::new(vec![
            Arq::new(pow, sb1, 8.into()).to_bounds(&topo),
            Arq::new(pow - 1, sb2, 8.into()).to_bounds(&topo),
        ]);

        let c = a.intersection(&topo, &b);
        print_arqs(&topo, &a, 64);
        println!();
        print_arqs(&topo, &b, 64);
        println!();
        // the last arq of c doesn't show up in the ascii representation, but
        // it is there.
        print_arqs(&topo, &c, 64);

        let arqs = c.arqs();
        assert_eq!(arqs.len(), 3);
        assert_eq!(arqs[0].start, 0.into());
        assert_eq!(arqs[1].start, 13.into());
        assert_eq!(arqs[2].start, 20.into());
    }

    #[test]
    fn normalize_arqs() {
        let s = ArqSet::new(vec![
            ArqBounds {
                start: 0.into(),
                power: 10,
                count: SpaceOffset(10),
            },
            ArqBounds {
                start: 0.into(),
                power: 8,
                count: SpaceOffset(40),
            },
            ArqBounds {
                start: 0.into(),
                power: 12,
                count: SpaceOffset(3),
            },
        ]);

        assert_eq!(
            s.arqs,
            vec![
                ArqBounds {
                    start: 0.into(),
                    power: 8,
                    count: SpaceOffset(4 * 10)
                },
                ArqBounds {
                    start: 0.into(),
                    power: 8,
                    count: SpaceOffset(40)
                },
                ArqBounds {
                    start: 0.into(),
                    power: 8,
                    count: SpaceOffset(3 * 16)
                },
            ]
        );
    }

    proptest::proptest! {
        #[test]
        fn rounded_arcset_intersections(p1 in 0u8..15, s1: u32, c1 in 8u32..64, p2 in 0u8..15, s2: u32, c2 in 8u32..64) {
            use crate::Loc;

            let topo = Topology::standard_epoch_full();
            let arq1 = Arq::new(p1, Loc::from(s1), c1.into());
            let arq2 = Arq::new(p2, Loc::from(s2), c2.into());
            let arcset1: DhtArcSet = arq1.to_bounds(&topo).to_dht_arc_range(&topo).into();
            let arcset2: DhtArcSet = arq2.to_bounds(&topo).to_dht_arc_range(&topo).into();
            let common = arcset1.intersection(&arcset2);
            let ii = common.intervals();
            for i in ii {
                let p = p1.min(p2);
                dbg!(&p, &i);
                let (_, rounded) = ArqBounds::from_interval_rounded(&topo, p, i);
                assert!(!rounded);
            }
        }
    }
}
