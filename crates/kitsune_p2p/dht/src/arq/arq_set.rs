use kitsune_p2p_dht_arc::DhtArcSet;

use crate::arq::ArqBounds;

/// A collection of ArqBounds.
/// All bounds are guaranteed to be quantized to the same power
/// (the lowest common power).
#[derive(Debug, Clone, PartialEq, Eq, derive_more::IntoIterator)]
pub struct ArqSet {
    #[into_iterator]
    pub(super) arqs: Vec<ArqBounds>,
    power: u8,
}

impl ArqSet {
    /// Normalize all arqs to be of the same power (use the minimum power)
    pub fn new(arqs: Vec<ArqBounds>) -> Self {
        if let Some(pow) = arqs.iter().map(|a| a.power).min() {
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

    pub fn single(arq: ArqBounds) -> Self {
        Self::new(vec![arq])
    }

    /// Get a reference to the arq set's power.
    pub fn power(&self) -> u8 {
        self.power
    }

    /// Get a reference to the arq set's arqs.
    pub fn arqs(&self) -> &[ArqBounds] {
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

    pub fn intersection(&self, other: &Self) -> Self {
        let power = self.power.min(other.power());
        let a1 = self.requantize(power).unwrap().to_dht_arc_set();
        let a2 = other.requantize(power).unwrap().to_dht_arc_set();
        Self {
            arqs: DhtArcSet::intersection(&a1, &a2)
                .intervals()
                .into_iter()
                .map(|interval| ArqBounds::from_interval(power, interval).expect("cannot fail"))
                .collect(),
            power,
        }
    }
}

/// View ascii for all arcs
pub fn print_arqs(arqs: &ArqSet, len: usize) {
    println!("{} arqs, power: {}", arqs.arqs().len(), arqs.power());
    for (i, arq) in arqs.arqs().into_iter().enumerate() {
        println!(
            "|{}| {}:\t{}",
            arq.to_interval().to_ascii(len),
            i,
            arq.count()
        );
    }
}

#[test]
fn normalize_arqs() {
    let s = ArqSet::new(vec![
        ArqBounds {
            offset: 0,
            power: 10,
            count: 10,
        },
        ArqBounds {
            offset: 0,
            power: 8,
            count: 40,
        },
        ArqBounds {
            offset: 0,
            power: 12,
            count: 3,
        },
    ]);

    assert_eq!(
        s.arqs,
        vec![
            ArqBounds {
                offset: 0,
                power: 8,
                count: 4 * 10
            },
            ArqBounds {
                offset: 0,
                power: 8,
                count: 40
            },
            ArqBounds {
                offset: 0,
                power: 8,
                count: 3 * 16
            },
        ]
    );
}
