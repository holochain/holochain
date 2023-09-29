#[derive(Default)]
struct Echo<T: Clone + Eq> {
    history: Vec<T>,
}

#[derive(PartialEq, Eq, Debug)]
pub enum Fx<T> {
    Msg(T),
}

// pub enum Ax<'a, T> {
//     Foo(&'a T),
// }

// impl<'a, T: 'a + Clone + Eq> stef::State<'a> for Echo<T> {
//     type Action = Ax<'a, T>;
//     type Effect = Fx<T>;

//     fn transition(&mut self, ax: Self::Action) -> Self::Effect {
//         todo!()
//     }
// }

#[stef_derive::state]
impl<'ax, T: 'ax + Clone + Eq> stef::State<'ax> for Echo<T> {
    type Action = Ax<'ax, T>;
    type Effect = Fx<T>;

    pub fn say(&mut self, what: &'ax T) -> Fx<T> {
        self.history.push(what.clone());
        Fx::Msg(what.clone())
    }
}

#[test]
fn test_generics() {
    let mut echo = Echo::default();
    assert_eq!(echo.say(&1), Fx::Msg(1));
    assert_eq!(echo.say(&2), Fx::Msg(2));
    assert_eq!(echo.history, vec![1, 2]);
}
