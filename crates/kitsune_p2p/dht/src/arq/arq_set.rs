use kitsune_p2p_dht_arc::DhtArcSet;

use crate::{
    arq::ArqBounds,
    quantum::{Offset, Topology},
    ArqStrat, Loc,
};

use super::{power_and_count_from_length, Arq, ArqStart};

pub type ArqSet = ArqSetImpl<Loc>;
pub type ArqBoundsSet = ArqSetImpl<Offset>;

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
pub struct ArqSetImpl<S: ArqStart> {
    #[into_iterator]
    #[deref]
    #[deref_mut]
    #[index]
    #[index_mut]
    #[serde(bound(deserialize = "S: serde::de::DeserializeOwned"))]
    pub(crate) arqs: Vec<Arq<S>>,
    power: u8,
}

impl<S: ArqStart> ArqSetImpl<S> {
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

    pub fn empty() -> Self {
        Self::new(vec![])
    }

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

    pub fn to_dht_arc_set(&self, topo: &Topology) -> DhtArcSet {
        DhtArcSet::from(
            self.arqs
                .iter()
                .map(|a| a.to_interval(topo))
                .collect::<Vec<_>>(),
        )
    }

    pub fn requantize(&self, power: u8) -> Option<Self> {
        self.arqs
            .iter()
            .map(|a| a.requantize(power))
            .collect::<Option<Vec<_>>>()
            .map(|arqs| Self { arqs, power })
    }

    pub fn intersection(&self, topo: &Topology, other: &Self) -> ArqSetImpl<Offset> {
        let power = self.power.min(other.power());
        let a1 = self.requantize(power).unwrap().to_dht_arc_set(topo);
        let a2 = other.requantize(power).unwrap().to_dht_arc_set(topo);
        ArqSetImpl {
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
        for (i, arq) in self.arqs().into_iter().enumerate() {
            println!("|{}| {:>3}:\t{:3}", arq.to_ascii(topo, len), i, arq.count(),);
        }
    }
}

impl ArqBoundsSet {
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
}

/// View ascii for arq bounds
pub fn print_arq<'a, S: ArqStart>(topo: &Topology, arq: &'a Arq<S>, len: usize) {
    println!(
        "|{}| {} *2^{}",
        arq.to_ascii(topo, len),
        arq.count(),
        arq.power()
    );
}

pub fn print_arqs<'a, S: ArqStart>(topo: &Topology, arqs: &'a [Arq<S>], len: usize) {
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

    use super::*;

    #[test]
    fn intersect_arqs() {
        observability::test_run().ok();
        let topo = Topology::unit_zero();
        let a = Arq::new(536870912u32.into(), 27, 11);
        let b = Arq::new(805306368u32.into(), 27, 11);
        dbg!(a.to_bounds(&topo).offset());

        let a = ArqSet::single(a);
        let b = ArqSet::single(b);
        let c = a.intersection(&topo, &b);
        print_arqs(&topo, &a, 64);
        print_arqs(&topo, &b, 64);
        print_arqs(&topo, &c, 64);
    }

    #[test]
    fn normalize_arqs() {
        let s = ArqSetImpl::new(vec![
            ArqBounds {
                start: 0.into(),
                power: 10,
                count: Offset(10),
            },
            ArqBounds {
                start: 0.into(),
                power: 8,
                count: Offset(40),
            },
            ArqBounds {
                start: 0.into(),
                power: 12,
                count: Offset(3),
            },
        ]);

        assert_eq!(
            s.arqs,
            vec![
                ArqBounds {
                    start: 0.into(),
                    power: 8,
                    count: Offset(4 * 10)
                },
                ArqBounds {
                    start: 0.into(),
                    power: 8,
                    count: Offset(40)
                },
                ArqBounds {
                    start: 0.into(),
                    power: 8,
                    count: Offset(3 * 16)
                },
            ]
        );
    }
}
