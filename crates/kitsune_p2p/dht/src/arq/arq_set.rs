use crate::arq::ArqBounds;

#[derive(Debug, Clone, PartialEq, Eq, derive_more::From, derive_more::IntoIterator)]
pub struct ArqSet(pub(super) Vec<ArqBounds>);

impl ArqSet {
    /// Normalize all arqs to be of the same power (use the minimum power)
    pub fn new(arqs: Vec<ArqBounds>) -> Self {
        if let Some(pow) = arqs.iter().map(|a| a.power).min() {
            Self(
                arqs.into_iter()
                    .map(|a| a.requantize(pow).unwrap())
                    .collect(),
            )
        } else {
            Self(vec![])
        }
    }

    pub fn coverage(lo: u32, hi: u32) {
        todo!("probably not needed")
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
        s,
        ArqSet(vec![
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
        ])
    );
}
