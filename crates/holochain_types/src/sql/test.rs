use std::ops::RangeInclusive;

use super::ToSqlStatement;
use holochain_zome_types::LinkType;
use holochain_zome_types::LinkTypeRange;
use holochain_zome_types::LinkTypeRanges;
use test_case::test_case;

#[test_case(LinkTypeRange::Empty => " false ".to_string())]
#[test_case(LinkTypeRange::Full => "".to_string())]
#[test_case(LinkTypeRange::Inclusive(LinkType(0)..=LinkType(0))=> " link_type = 0 ".to_string())]
#[test_case(LinkTypeRange::Inclusive(LinkType(1)..=LinkType(0))=> " false ".to_string())]
#[test_case(LinkTypeRange::Inclusive(LinkType(5)..=LinkType(6))=> " link_type BETWEEN 5 AND 6 ".to_string())]
fn link_type_range_to_sql(range: LinkTypeRange) -> String {
    range.to_sql_statement()
}

fn make_ranges(ranges: Vec<RangeInclusive<u8>>) -> Vec<LinkTypeRange> {
    ranges
        .into_iter()
        .map(|r| LinkTypeRange::Inclusive(LinkType(*r.start())..=LinkType(*r.end())))
        .collect()
}

#[test_case(vec![LinkTypeRange::Empty] => " AND false ".to_string())]
#[test_case(vec![LinkTypeRange::Empty; 4] => " AND false ".to_string())]
#[test_case(vec![LinkTypeRange::Full] => "".to_string())]
#[test_case(vec![LinkTypeRange::Full; 4] => "".to_string())]
#[test_case(make_ranges(vec![0..=0]) => " AND (  link_type = 0  ) ".to_string())]
#[test_case(make_ranges(vec![0..=0, 0..=0]) => " AND (  link_type = 0  ) ".to_string())]
#[test_case(make_ranges(vec![10..=3]) => " AND false ".to_string())]
#[test_case(make_ranges(vec![10..=3, 0..=1]) => " AND false ".to_string())]
#[test_case(make_ranges(vec![10..=3, 5..=1]) => " AND false ".to_string())]
#[test_case(make_ranges(vec![0..=u8::MAX]) => "".to_string())]
#[test_case(make_ranges(vec![0..=u8::MAX, 5..=1]) => " AND false ".to_string())]
#[test_case(make_ranges(vec![0..=u8::MAX, 1..=1]) => " AND (  link_type = 1  ) ".to_string())]
#[test_case(make_ranges(vec![0..=u8::MAX, 1..=1, 5..=5]) => " AND (  link_type = 1  OR  link_type = 5  ) ".to_string())]
#[test_case(make_ranges(vec![0..=u8::MAX, 1..=1, 5..=6]) => " AND (  link_type = 1  OR  link_type BETWEEN 5 AND 6  ) ".to_string())]
#[test_case(make_ranges(vec![0..=5, 30..=50, 7..=9]) => " AND (  link_type BETWEEN 0 AND 5  OR  link_type BETWEEN 30 AND 50  OR  link_type BETWEEN 7 AND 9  ) ".to_string())]
fn link_type_ranges_to_sql(ranges: Vec<LinkTypeRange>) -> String {
    LinkTypeRanges(ranges).to_sql_statement()
}
