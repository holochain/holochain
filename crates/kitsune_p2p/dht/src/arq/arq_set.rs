//! Types representing a set of Arqs all of the same "power".

use kitsune_p2p_dht_arc::DhtArcSet;

use crate::{arq::ArqBounds, spacetime::*};

use super::{Arq, ArqStart};

/// A collection of ArqBounds.
/// All bounds are guaranteed to be quantized to the same power
/// (the lowest common power).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
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

    /// Singleton set
    #[cfg(feature = "test_utils")]
    pub fn full_std() -> Self {
        use crate::ArqStrat;

        Self::new(vec![Arq::<S>::new_full_max(
            SpaceDimension::standard(),
            &ArqStrat::default(),
            S::zero(),
        )])
    }

    /// Get a reference to the arq set's power.
    pub fn power(&self) -> u8 {
        self.power
    }

    /// Get a reference to the arq set's arqs.
    pub fn arqs(&self) -> &[Arq<S>] {
        self.arqs.as_ref()
    }

    /// Convert to a set of "continuous" arcs using standard topology
    pub fn to_dht_arc_set_std(&self) -> DhtArcSet {
        self.to_dht_arc_set(SpaceDimension::standard())
    }

    /// Convert to a set of "continuous" arcs
    pub fn to_dht_arc_set(&self, dim: impl SpaceDim) -> DhtArcSet {
        DhtArcSet::from(
            self.arqs
                .iter()
                .map(|a| a.to_dht_arc_range(dim))
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
    pub fn intersection(&self, dim: impl SpaceDim, other: &Self) -> ArqSet<SpaceOffset> {
        let power = self.power.min(other.power());
        let a1 = self.requantize(power).unwrap().to_dht_arc_set(dim);
        let a2 = other.requantize(power).unwrap().to_dht_arc_set(dim);
        ArqSet {
            arqs: DhtArcSet::intersection(&a1, &a2)
                .intervals()
                .into_iter()
                .map(|interval| {
                    ArqBounds::from_interval(dim, power, interval).expect("cannot fail")
                })
                .collect(),
            power,
        }
    }

    /// View ascii for all arq bounds
    #[cfg(feature = "test_utils")]
    pub fn print_arqs(&self, dim: impl SpaceDim, len: usize) {
        println!("{} arqs, power: {}", self.arqs().len(), self.power());
        for (i, arq) in self.arqs().iter().enumerate() {
            println!(
                "{:>3}: |{}| {} {}/{} @ {:?}",
                i,
                arq.to_ascii(dim, len),
                arq.absolute_length(dim),
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
    /// This is necessary because an arcset which is the union of many agents'
    /// arcs may be much longer than any one agent's arcs, which would not quantize
    /// properly to an arq that fits the bounds of the ArqStrat (num chunks between
    /// 8 and 16), so if we want an exact match (which we often do!) we need to
    /// allow the power to be lower and the chunk size to be greater to provide
    /// the exact match.
    //
    // TODO: XXX: revisit this when power levels really matter, because this
    //   does entail a loss of info about the original power levels of the original
    //   arqs, or even of the original arqset minimum power level. For instance we
    //   may need to refactor agent info to include power level so as not to lose
    //   this info.
    #[cfg(feature = "test_utils")]
    pub fn from_dht_arc_set_exact(
        dim: impl SpaceDim,
        strat: &crate::ArqStrat,
        dht_arc_set: &DhtArcSet,
    ) -> Option<Self> {
        Some(Self::new(
            dht_arc_set
                .intervals()
                .into_iter()
                .map(|i| {
                    let len = i.length();
                    let super::ArqSize { power, .. } =
                        super::power_and_count_from_length_exact(dim, len, strat.min_chunks())?;
                    ArqBounds::from_interval(dim, power, i)
                })
                .collect::<Option<Vec<_>>>()?,
        ))
    }
}

/// Print ascii for arq bounds
#[cfg(feature = "test_utils")]
pub fn print_arq<S: ArqStart>(dim: impl SpaceDim, arq: &Arq<S>, len: usize) {
    println!(
        "|{}| {} *2^{}",
        arq.to_ascii(dim, len),
        arq.count(),
        arq.power()
    );
}

/// Print a collection of arqs
#[cfg(feature = "test_utils")]
pub fn print_arqs<S: ArqStart>(dim: impl SpaceDim, arqs: &[Arq<S>], len: usize) {
    for (i, arq) in arqs.iter().enumerate() {
        println!(
            "|{}| {}:\t{} +{} *2^{}",
            arq.to_ascii(dim, len),
            i,
            *arq.start.to_offset(dim, arq.power()),
            arq.count(),
            arq.power()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{prelude::pow2, ArqStrat};
    use kitsune_p2p_dht_arc::DhtArcRange;

    #[test]
    fn intersect_arqs() {
        holochain_trace::test_run();
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
        holochain_trace::test_run();
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

    #[test]
    fn arq_set_is_union() {
        let dim = SpaceDimension::standard();
        let strat = ArqStrat::default();

        let start_a = 10_000;
        let len_a = 30_000;
        let arq_a =
            Arq::from_start_and_half_len_approximate(dim, &strat, start_a.into(), len_a / 2);

        let start_b = 10_000_000;
        let len_b = 20_000;
        let arq_b =
            Arq::from_start_and_half_len_approximate(dim, &strat, start_b.into(), len_b / 2);

        let arq_set = ArqSet::new(vec![arq_a.to_bounds_std(), arq_b.to_bounds_std()]);
        let arc_set = arq_set.to_dht_arc_set_std();

        // Before first interval
        {
            let agent_arc_before_a =
                Arq::from_start_and_half_len_approximate(dim, &strat, 100.into(), 100);
            let interval = DhtArcRange::from(agent_arc_before_a.to_dht_arc_std());

            assert!(!arc_set.overlap(&interval.into()));
        }

        // Overlaps with start of first interval
        {
            let agent_arc_overlap_start_a = Arq::from_start_and_half_len_approximate(
                dim,
                &strat,
                (start_a - 1_000).into(),
                1_500,
            );
            let interval = DhtArcRange::from(agent_arc_overlap_start_a.to_dht_arc_std());

            assert!(arc_set.overlap(&interval.into()));
        }

        // Inside first interval
        {
            let agent_arc_inside_a = Arq::from_start_and_half_len_approximate(
                dim,
                &strat,
                (start_a + 100).into(),
                len_a / 10,
            );
            let interval = DhtArcRange::from(agent_arc_inside_a.to_dht_arc_std());

            assert!(arc_set.overlap(&interval.into()));
        }

        // Overlaps with the end of the first interval
        {
            let agent_arc_overlap_end_a = Arq::from_start_and_half_len_approximate(
                dim,
                &strat,
                (start_a + len_a - 1_000).into(),
                1_500,
            );
            let interval = agent_arc_overlap_end_a.to_dht_arc_range_std();

            assert!(arc_set.overlap(&interval.into()));
        }

        // Between the two intervals
        {
            let agent_arc_between =
                Arq::from_start_and_half_len_approximate(dim, &strat, 1_000_000.into(), 1_000);
            let interval = DhtArcRange::from(agent_arc_between.to_dht_arc_std());

            assert!(!arc_set.overlap(&interval.into()));
        }

        // Overlap with the start of the second interval
        {
            let agent_arc_overlap_start_b = Arq::from_start_and_half_len_approximate(
                dim,
                &strat,
                (start_b - 10_000).into(),
                15_000,
            );
            let interval = DhtArcRange::from(agent_arc_overlap_start_b.to_dht_arc_std());

            assert!(arc_set.overlap(&interval.into()));
        }
    }

    proptest::proptest! {

    #[test]
    fn arqset_intersection_smoke(
        p1 in 12u8..17, s1: u32, c1: u32,
        p2 in 12u8..17, s2: u32, c2: u32,
        p3 in 12u8..17, s3: u32, c3: u32,
        p4 in 12u8..17, s4: u32, c4: u32,
        p5 in 12u8..17, s5: u32, c5: u32,
        p6 in 12u8..17, s6: u32, c6: u32,
    ) {
        use crate::Loc;

        let topo = Topology::standard_epoch_full();
        let strat = ArqStrat::default();

        let c1 = strat.min_chunks() + c1 % (strat.max_chunks() - strat.min_chunks());
        let c2 = strat.min_chunks() + c2 % (strat.max_chunks() - strat.min_chunks());
        let c3 = strat.min_chunks() + c3 % (strat.max_chunks() - strat.min_chunks());
        let c4 = strat.min_chunks() + c4 % (strat.max_chunks() - strat.min_chunks());
        let c5 = strat.min_chunks() + c5 % (strat.max_chunks() - strat.min_chunks());
        let c6 = strat.min_chunks() + c6 % (strat.max_chunks() - strat.min_chunks());

        let arq1 = Arq::new(p1, Loc::from(s1), c1.into());
        let arq2 = Arq::new(p2, Loc::from(s2), c2.into());
        let arq3 = Arq::new(p3, Loc::from(s3), c3.into());
        let arq4 = Arq::new(p4, Loc::from(s4), c4.into());
        let arq5 = Arq::new(p5, Loc::from(s5), c5.into());
        let arq6 = Arq::new(p6, Loc::from(s6), c6.into());

        let arcset1 = ArqSet::new(vec![ arq1, arq2, arq3 ]);
        let arcset2 = ArqSet::new(vec![ arq4, arq5, arq6 ]);

        // This can panic
        arcset1.intersection(&topo, &arcset2);

    }
    }
}
