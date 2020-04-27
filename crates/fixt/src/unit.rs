use crate::prelude::*;

type Unit = ();
fixturator!(Unit, (), (), ());
basic_test!(Unit, vec![(); 40], vec![(); 40], false);
