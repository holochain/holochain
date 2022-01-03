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

    /// Get a reference to the arq set's power.
    pub fn power(&self) -> u8 {
        self.power
    }

    /// Get a reference to the arq set's arqs.
    pub fn arqs(&self) -> &[ArqBounds] {
        self.arqs.as_ref()
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
