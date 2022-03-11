//! Allows easy construction of small ranges via ASCII art, useful for testing

use crate::{DhtArc, DhtArcSet};

pub fn ascii(s: &str) -> DhtArcSet {
    let mut arcs = Vec::<DhtArc>::new();
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
        arcs.push(DhtArc::from_bounds(start as u32, end as u32));
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
                    DhtArc::from_bounds(0, 2).canonical(),
                    DhtArc::from_bounds(u32::MAX - 2, u32::MAX).canonical()
                ]
                .as_slice()
            )
            .intervals(),
            vec![DhtArc::from_bounds(u32::MAX - 2, 2).canonical()]
        );
        assert_eq!(
            ascii("ooo    oo ").intervals(),
            vec![
                DhtArc::from_bounds(0, 2).canonical(),
                DhtArc::from_bounds(7, 8).canonical()
            ]
        );
        assert_eq!(
            ascii("oo oo o   ").intervals(),
            vec![
                DhtArc::from_bounds(0, 1).canonical(),
                DhtArc::from_bounds(3, 4).canonical(),
                DhtArc::from_bounds(6, 6).canonical(),
            ]
        );
    }
}
