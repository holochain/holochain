use crate::dht_arc::loc_upscale;

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn correct_ascii() {
    test_cases(
        16,
        [
            (0, 8, "----@----       "),
            (1, 8, " ----@---       "),
            (2, 8, "  ---@---       "),
            (8, 0, "-       ----@---"),
            (8, 1, "--      -----@--"),
            (8, 2, "---     -----@--"),
            (9, 5, "------   ------@"),
            (9, 6, "@------  -------"),
            (9, 7, "@------- -------"),
        ],
    );
}

fn test_cases<'a>(len: usize, cases: impl IntoIterator<Item = (i32, i32, &'a str)> + Clone) {
    let fmt_bounds = |lo, hi| format!("({:+}, {:+})", lo, hi);

    let expected: Vec<_> = cases
        .clone()
        .into_iter()
        .map(|(lo, hi, ascii)| (fmt_bounds(lo, hi), ascii.to_string()))
        .collect();

    let actual: Vec<_> = cases
        .into_iter()
        .map(|(lo, hi, _)| {
            let ascii = ArcInterval::new(loc_upscale(len, lo), loc_upscale(len, hi)).to_ascii(len);
            let bounds = fmt_bounds(lo, hi);
            assert_eq!(ascii.len(), len, "Wrong length for case {}", bounds);
            (bounds, ascii)
        })
        .collect();

    assert_eq!(expected, actual);
}
