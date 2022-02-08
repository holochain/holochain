//! Allows easy construction of small ranges via ASCII art, useful for testing

use crate::{ArcInterval, DhtArcSet};

pub fn ascii(s: &str) -> DhtArcSet {
    let mut arcs = Vec::<ArcInterval>::new();
    let mut i: usize = 0;

    loop {
        if i >= s.len() {
            break;
        }
        while i < s.len() && &s[i..=i] == " " {
            i += 1
        }
        if i >= s.len() {
            break;
        }
        let start = i;
        while i < s.len() && &s[i..=i] != " " {
            i += 1
        }
        let end = i - 1;
        arcs.push(ArcInterval::new(start as u32, end as u32));
    }

    arcs.as_slice().into()
}

#[cfg(test)]
mod tests {

    use crate::DhtArcSet;

    use super::*;

    #[test]
    // @maackle Do you know why this is now failing?
    #[ignore = "Broken not sure how to fix"]
    fn sanity() {
        assert_eq!(
            DhtArcSet::from(
                vec![
                    ArcInterval::new(0, 2).canonical(),
                    ArcInterval::new(u32::MAX - 2, u32::MAX).canonical()
                ]
                .as_slice()
            )
            .intervals(),
            vec![ArcInterval::new(u32::MAX - 2, 2).canonical()]
        );
        assert_eq!(
            ascii("ooo    oo ").intervals(),
            vec![
                ArcInterval::new(0, 2).canonical(),
                ArcInterval::new(7, 8).canonical()
            ]
        );
        assert_eq!(
            ascii("oo oo o   ").intervals(),
            vec![
                ArcInterval::new(0, 1).canonical(),
                ArcInterval::new(3, 4).canonical(),
                ArcInterval::new(6, 6).canonical(),
            ]
        );
    }
}
