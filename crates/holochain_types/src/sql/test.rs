use super::ToSqlStatement;
use holochain_zome_types::LinkType;
use holochain_zome_types::LinkTypeFilter;
use holochain_zome_types::ZomeIndex;
use test_case::test_case;

fn make_multi(types: &[(u8, &[u8])]) -> LinkTypeFilter {
    LinkTypeFilter::Types(
        types
            .iter()
            .map(|(z, t)| (ZomeIndex(*z), t.iter().map(|t| LinkType(*t)).collect()))
            .collect(),
    )
}

#[test_case(make_multi(&[]) => "".to_string())]
#[test_case(make_multi(&[(0, &[0])]) => " AND zome_index = 0 AND link_type = 0 ".to_string())]
#[test_case(make_multi(&[(0, &[0]), (1, &[0, 1])]) => " AND ( ( zome_index = 0 AND ( link_type = 0 ) ) OR ( zome_index = 1 AND ( link_type = 0 OR link_type = 1 ) ) ) ".to_string())]
#[test_case(make_multi(&[(0, &[0, 2, 5])]) => " AND ( ( zome_index = 0 AND ( link_type = 0 OR link_type = 2 OR link_type = 5 ) ) ) ".to_string())]
#[test_case(LinkTypeFilter::Dependencies(vec![]) => "".to_string())]
#[test_case(LinkTypeFilter::Dependencies(vec![ZomeIndex(0)]) => " AND ( zome_index = 0 ) ".to_string())]
#[test_case(LinkTypeFilter::Dependencies(vec![ZomeIndex(0), ZomeIndex(3)]) => " AND ( zome_index = 0 OR zome_index = 3 ) ".to_string())]
fn link_type_filter_to_sql(filter: LinkTypeFilter) -> String {
    filter.to_sql_statement()
}

#[test_case(make_multi(&[]), 0, 0 => false)]
#[test_case(make_multi(&[(0, &[0])]), 0, 0 => true)]
#[test_case(make_multi(&[(0, &[0])]), 1, 0 => false)]
#[test_case(make_multi(&[(0, &[0])]), 1, 1 => false)]
#[test_case(make_multi(&[(0, &[0])]), 0, 1 => false)]
#[test_case(make_multi(&[(0, &[0]), (1, &[0, 1])]), 0, 0 => true)]
#[test_case(make_multi(&[(0, &[0]), (1, &[0, 1])]), 1, 0 => true)]
#[test_case(make_multi(&[(0, &[0]), (1, &[0, 1])]), 1, 1 => true)]
#[test_case(make_multi(&[(0, &[0]), (1, &[0, 1])]), 1, 2 => false)]
#[test_case(make_multi(&[(0, &[0]), (1, &[0, 1])]), 2, 0 => false)]
#[test_case(make_multi(&[(0, &[0]), (1, &[0, 1])]), 0, 1 => false)]
#[test_case(make_multi(&[(0, &[0, 2, 5])]), 0, 5 => true)]
#[test_case(make_multi(&[(0, &[0, 2, 5])]), 0, 6 => false)]
#[test_case(make_multi(&[(0, &[0, 2, 5])]), 1, 5 => false)]
#[test_case(LinkTypeFilter::Dependencies(vec![]), 0, 0 => false)]
#[test_case(LinkTypeFilter::Dependencies(vec![ZomeIndex(0)]), 0, 0 => true)]
#[test_case(LinkTypeFilter::Dependencies(vec![ZomeIndex(0)]), 0, 1 => true)]
#[test_case(LinkTypeFilter::Dependencies(vec![ZomeIndex(0)]), 1, 0 => false)]
#[test_case(LinkTypeFilter::Dependencies(vec![ZomeIndex(0), ZomeIndex(3)]), 0, 0 => true)]
#[test_case(LinkTypeFilter::Dependencies(vec![ZomeIndex(0), ZomeIndex(3)]), 3, 0 => true)]
#[test_case(LinkTypeFilter::Dependencies(vec![ZomeIndex(0), ZomeIndex(3)]), 2, 0 => false)]
#[test_case(LinkTypeFilter::Dependencies(vec![ZomeIndex(0), ZomeIndex(3)]), 4, 0 => false)]
fn link_type_filter_contains(filter: LinkTypeFilter, z: u8, l: u8) -> bool {
    filter.contains(&ZomeIndex(z), &LinkType(l))
}
