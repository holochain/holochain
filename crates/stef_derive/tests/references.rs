use std::marker::PhantomData;

#[derive(Default)]
struct Echo<'a, T: Clone + Eq> {
    history: Vec<T>,
    _phantom: PhantomData<&'a ()>,
}

#[derive(PartialEq, Eq, Debug)]
pub enum Fx<'a, T> {
    Msg(&'a T),
}

#[stef_derive::state]
impl<'a, T: Clone + Eq> stef::State<'a> for Echo<'a, T>
where
    Self: 'a,
{
    type Action = Ax<'a, T>;
    type Effect = Fx<'a, T>;

    pub fn say(&mut self, what: &'a T) -> Fx<'a, T> {
        self.history.push(what.clone());
        Fx::Msg(what)
    }
}

#[test]
fn test_generics() {
    let mut echo = Echo::default();
    assert_eq!(echo.say(&1), Fx::Msg(&1));
    assert_eq!(echo.say(&2), Fx::Msg(&2));
    assert_eq!(echo.history, vec![1, 2]);
}
