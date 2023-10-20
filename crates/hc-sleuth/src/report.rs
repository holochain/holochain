use crate::*;

pub type Report = Vec<ReportItem>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportItem {
    Line(String),
    Fork(Vec<Report>),
}

#[macro_export]
macro_rules! report_item {
    ( [ $( [ $($f:expr),+ ] ),+ ] ) => {
        $crate::ReportItem::Fork(vec![
            $(
                vec![ $(
                    report_item![ $f ]
                ),+ ]
            ),+
        ])
    };
    ( $f:expr ) => {
        {
            use $crate::Fact;
            $crate::ReportItem::Line($f.explain())
        }
    };

}

#[macro_export]
macro_rules! report {
    ( $( $f:expr ),* ) => {
        vec![ $( $crate::report_item![$f] ),* ]
    };

}

#[test]
fn report_item() {
    use crate::test_fact::F;
    use pretty_assertions::assert_eq;

    let f = |id| F::new(id, false, ());

    assert_eq!(report_item!(f(1)), ReportItem::Line(f(1).explain()));

    assert_eq!(
        report_item!([[f(1)], [f(2), f(3)]]),
        ReportItem::Fork(vec![
            vec![ReportItem::Line(f(1).explain())],
            vec![
                ReportItem::Line(f(2).explain()),
                ReportItem::Line(f(3).explain())
            ]
        ])
    );
}

#[test]
fn reports() {
    use crate::test_fact::F;
    use pretty_assertions::assert_eq;

    let f = |id| F::new(id, false, ());

    let report1 = vec![
        ReportItem::Line(f(11).explain()),
        ReportItem::Line(f(12).explain()),
        ReportItem::Line(f(13).explain()),
    ];
    let report2 = vec![
        ReportItem::Line(f(21).explain()),
        ReportItem::Line(f(22).explain()),
    ];
    let report3 = vec![
        ReportItem::Line(f(31).explain()),
        ReportItem::Fork(vec![report1.clone(), report2.clone()]),
    ];

    assert_eq!(report1, report![f(11), f(12), f(13)]);
    assert_eq!(report2, report![f(21), f(22), f(23)]);

    let item = report_item!([[f(11), f(12), f(13)], [f(21), f(22)]]);
    let expected3 = report![f(31), [[f(11), f(12), f(13)], [f(21), f(22)]]];
    assert_eq!(dbg!(report3), dbg!(expected3));
}
