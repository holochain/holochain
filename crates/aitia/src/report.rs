use std::{collections::HashSet, fmt::Write};

use crate::*;

#[derive(Debug, Default, Clone, PartialEq, Eq, derive_more::Into, derive_more::Deref)]
pub struct Report {
    #[deref]
    #[into]
    tree: Vec<ReportItem>,
    visited: HashSet<String>,
}

fn bullet(indent: usize) -> String {
    format!("{}-", "  ".repeat(indent))
}

impl Report {
    pub fn new(tree: Vec<ReportItem>) -> Self {
        // TODO: construct visited map
        Self {
            tree,
            visited: Default::default(),
        }
        // let mut visited = HashSet::new();
        // for item in tree.iter() {
        //     match item {
        //         ReportItem::Line(line) => visited.add(line),
        //         ReportItem::Fork(_) => todo!(),
        //     }
        // }
    }

    pub fn push(&mut self, item: ReportItem) {
        todo!()
        // match item {
        //     ReportItem::Line(line) => {
        //         self.visited.insert(line.clone());
        //     }
        //     ReportItem::Fork(rs) => {
        //         for mut r in rs {
        //             for item in r.iter() {
        //                 // self
        //                 if self.visited.insert(item.clone()) {
        //                     self.tree.push(item);
        //                 } else {
        //                     continue;
        //                 }
        //             }
        //         }
        //     }
        // }
        // self.tree.push(item);
    }

    pub fn pretty(&self) -> String {
        let mut out = "\n".to_string();
        self.write(&mut out, 0);
        out
    }

    fn write(&self, out: &mut impl Write, indent: usize) {
        for item in self.tree.iter().rev() {
            match item {
                ReportItem::Line(line) => {
                    writeln!(out, "{} {}", bullet(indent), line);
                }
                ReportItem::Fork(rs) => {
                    for (i, r) in rs.iter().enumerate() {
                        writeln!(out, "{} fork {}:", bullet(indent + 1), i);
                        r.write(out, indent + 2);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportItem {
    Line(String),
    Fork(Vec<Report>),
}

#[macro_export]
macro_rules! item {
    ( $( [ $($f:expr),+ ] ),+ ) => {
        $crate::ReportItem::Fork(vec![
            $(
                Report::from(vec![ $(
                    item![ $f ]
                ),+ ])
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
    ( $( $f:expr ),* ) => {{
        use $crate::Fact;
        $crate::Report::from(vec![ $( $crate::item!($f) ),* ])
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item() {
        use crate::test_fact::F;
        use pretty_assertions::assert_eq;

        let f = |id| F::new(id, false, ());

        assert_eq!(item!(f(1)), ReportItem::Line(f(1).explain()));

        assert_eq!(
            item!([f(1)], [f(2), f(3)]),
            ReportItem::Fork(vec![
                vec![ReportItem::Line(f(1).explain())].into(),
                vec![
                    ReportItem::Line(f(2).explain()),
                    ReportItem::Line(f(3).explain())
                ]
                .into()
            ])
        );
    }

    #[test]
    fn reports() {
        use crate::test_fact::F;
        use pretty_assertions::assert_eq;

        let f = |id| F::new(id, false, ());

        let report1 = Report::from(vec![
            ReportItem::Line(f(11).explain()),
            ReportItem::Line(f(12).explain()),
            ReportItem::Line(f(13).explain()),
        ]);
        let report2 = Report::from(vec![
            ReportItem::Line(f(21).explain()),
            ReportItem::Line(f(22).explain()),
        ]);
        let report3 = Report::from(vec![
            ReportItem::Fork(vec![report1.clone().into(), report2.clone().into()]),
            ReportItem::Line(f(31).explain()),
        ]);

        assert_eq!(report1, report![f(11), f(12), f(13)]);
        assert_eq!(report2, report![f(21), f(22)]);

        let item = item!([f(11), f(12), f(13)], [f(21), f(22)]);
        let expected3 = Report::from(vec![
            item!([f(11), f(12), f(13)], [f(21), f(22)]),
            item!(f(31)),
        ]);
        assert_eq!(report3, expected3);
        assert_eq!(
            report3.pretty(),
            r"
- 31
  - fork 0:
    - 13
    - 12
    - 11
  - fork 1:
    - 22
    - 21
"
        );
    }
}
