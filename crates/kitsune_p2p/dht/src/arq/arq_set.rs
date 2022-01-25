use kitsune_p2p_dht_arc::DhtArcSet;

use crate::arq::ArqBounds;

use super::{Arq, ArqBounded};

pub type ArqSet = ArqSetImpl<Arq>;
pub type ArqBoundsSet = ArqSetImpl<ArqBounds>;

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
)]
pub struct ArqSetImpl<A: ArqBounded> {
    #[into_iterator]
    #[deref]
    #[deref_mut]
    #[index]
    #[index_mut]
    pub(crate) arqs: Vec<A>,
    power: u8,
}

impl<A: ArqBounded> ArqSetImpl<A> {
    /// Normalize all arqs to be of the same power (use the minimum power)
    pub fn new(arqs: Vec<A>) -> Self {
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

    pub fn single(arq: A) -> Self {
        Self::new(vec![arq])
    }

    /// Get a reference to the arq set's power.
    pub fn power(&self) -> u8 {
        self.power
    }

    /// Get a reference to the arq set's arqs.
    pub fn arqs(&self) -> &[A] {
        self.arqs.as_ref()
    }

    pub fn to_dht_arc_set(&self) -> DhtArcSet {
        DhtArcSet::from(
            self.arqs
                .iter()
                .map(|a| a.to_interval())
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

    pub fn intersection(&self, other: &Self) -> ArqSetImpl<ArqBounds> {
        let power = self.power.min(other.power());
        let a1 = self.requantize(power).unwrap().to_dht_arc_set();
        let a2 = other.requantize(power).unwrap().to_dht_arc_set();
        ArqSetImpl {
            arqs: DhtArcSet::intersection(&a1, &a2)
                .intervals()
                .into_iter()
                .map(|interval| ArqBounds::from_interval(power, interval).expect("cannot fail"))
                .collect(),
            power,
        }
    }

    /// View ascii for all arq bounds
    pub fn print_arqs(&self, len: usize) {
        println!("{} arqs, power: {}", self.arqs().len(), self.power());
        for (i, arq) in self.arqs().into_iter().enumerate() {
            println!("|{}| {}:\t{}", arq.to_ascii(len), i, arq.count());
        }
    }
}

/// View ascii for arq bounds
pub fn print_arq<'a, A: ArqBounded>(arq: &'a A, len: usize) {
    println!("|{}| {} *2^{}", arq.to_ascii(len), arq.count(), arq.power());
}

pub fn print_arqs<'a, A: ArqBounded>(arqs: &'a [A], len: usize) {
    for (i, arq) in arqs.iter().enumerate() {
        println!(
            "|{}| {}:\t{} +{} *2^{}",
            arq.to_ascii(len),
            i,
            arq.to_bounds().offset(),
            arq.count(),
            arq.power()
        );
    }
}

mod tests {

    use super::*;

    #[test]
    fn intersect_arqs() {
        observability::test_run().ok();
        let a = Arq::new(536870912.into(), 27, 11);
        let b = Arq::new(805306368.into(), 27, 11);
        dbg!(a.to_bounds().offset());

        let a = ArqSet::single(a);
        let b = ArqSet::single(b);
        let c = a.intersection(&b);
        print_arqs(&a, 64);
        print_arqs(&b, 64);
        print_arqs(&c, 64);
    }

    #[test]
    fn normalize_arqs() {
        let s = ArqSetImpl::new(vec![
            ArqBounds {
                offset: 0.into(),
                power: 10,
                count: 10,
            },
            ArqBounds {
                offset: 0.into(),
                power: 8,
                count: 40,
            },
            ArqBounds {
                offset: 0.into(),
                power: 12,
                count: 3,
            },
        ]);

        assert_eq!(
            s.arqs,
            vec![
                ArqBounds {
                    offset: 0.into(),
                    power: 8,
                    count: (4 * 10)
                },
                ArqBounds {
                    offset: 0.into(),
                    power: 8,
                    count: 40
                },
                ArqBounds {
                    offset: 0.into(),
                    power: 8,
                    count: (3 * 16)
                },
            ]
        );
    }
}
