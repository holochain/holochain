use crate::ArcInterval;

use pretty_assertions::assert_eq;

#[test]
fn correct_ascii() {
    test_cases_scaled(
        16,
        [
            (0, 8, "----@----       "),
            (1, 8, " ---@----       "),
            (2, 8, "  ---@---       "),
            (8, 0, "-       ----@---"),
            (8, 1, "--      ----@---"),
            (8, 2, "---     -----@--"),
            (9, 5, "------   ------@"),
            (9, 6, "-------  ------@"),
            (9, 7, "@------- -------"),
            (8, 7, "---------------@"),
            (1, 0, "--------@-------"),
            (0, 15, "-------@--------"),
        ],
    );
}

#[test]
fn correct_ascii_regressions() {
    test_cases_raw(
        32,
        [
            (17331162, 4277636136, "----------------@---------------"),
            (82641416, 4212325882, "----------------@---------------"),
            (2213596780, 2081370516, "@-------------------------------"),
            (3832356631, 4267951689, "                            --@-"),
        ],
    );
}

fn test_cases_raw<'a>(len: usize, cases: impl IntoIterator<Item = (u32, u32, &'a str)> + Clone) {
    let fmt_bounds = |lo, hi| format!("({:+}, {:+})", lo, hi);

    let expected: Vec<_> = cases
        .clone()
        .into_iter()
        .map(|(lo, hi, ascii)| (fmt_bounds(lo, hi), ascii.to_string()))
        .collect();

    let actual: Vec<_> = cases
        .into_iter()
        .map(|(lo, hi, _)| {
            let ascii = ArcInterval::new(lo, hi).to_ascii(len);
            let bounds = fmt_bounds(lo, hi);
            assert_eq!(ascii.len(), len, "Wrong length for case {}", bounds);
            (bounds, ascii)
        })
        .collect();

    assert_eq!(expected, actual);
}

fn test_cases_scaled<'a>(len: usize, cases: impl IntoIterator<Item = (i32, i32, &'a str)> + Clone) {
    let fmt_bounds = |lo, hi| format!("({:+}, {:+})", lo, hi);

    let expected: Vec<_> = cases
        .clone()
        .into_iter()
        .map(|(lo, hi, ascii)| (fmt_bounds(lo, hi), ascii.to_string()))
        .collect();

    let actual: Vec<_> = cases
        .into_iter()
        .map(|(lo, hi, _)| {
            let ascii = ArcInterval::new(lo, hi).to_ascii(len);
            let bounds = fmt_bounds(lo, hi);
            assert_eq!(ascii.len(), len, "Wrong length for case {}", bounds);
            (bounds, ascii)
        })
        .collect();

    assert_eq!(expected, actual);
}
