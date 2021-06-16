//! Allows easy construction of small ranges via ASCII art, useful for testing

use crate::{ArcInterval, DhtArcSet};

pub fn ascii(s: &str) -> DhtArcSet {
    let mut wints = Vec::<ArcInterval>::new();
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
        wints.push(ArcInterval::new(start as u32, end as u32));
    }

    wints.into()
}

#[cfg(test)]
mod tests {

    use crate::DhtArcSet;

    use super::*;

    #[test]
    fn sanity() {
        assert_eq!(
            DhtArcSet::from(vec![
                ArcInterval::new(0, 2),
                ArcInterval::new(u32::MAX - 2, u32::MAX)
            ])
            .intervals(),
            vec![ArcInterval::new(u32::MAX - 2, 2)]
        );
        assert_eq!(
            ascii("ooo    oo ").intervals(),
            vec![ArcInterval::new(0, 2), ArcInterval::new(7, 8)]
        );
        assert_eq!(
            ascii("oo oo o   ").intervals(),
            vec![
                ArcInterval::new(0, 1),
                ArcInterval::new(3, 4),
                ArcInterval::new(6, 6)
            ]
        );
    }
}
