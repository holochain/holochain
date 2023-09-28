#[allow(dead_code)]
#[derive(Debug)]
struct Person<D: std::fmt::Debug> {
    name: String,
    age: u8,
    data: D,
}

#[derive(Default, Debug)]
pub struct People<D: std::fmt::Debug>(Vec<Person<D>>);

#[derive(PartialEq, Eq, Debug)]
pub enum PeopleFx<D> {
    SayHi(String, D),
}

#[derive(derive_more::Deref, stef::State)]
pub struct PeopleShare<D: std::fmt::Debug>(stef::Share<People<D>>);

impl<D: 'static + Clone + Eq + std::fmt::Debug> From<People<D>> for PeopleShare<D> {
    fn from(value: People<D>) -> Self {
        Self(stef::Share::new(value))
    }
}

#[stef_derive::state(wrapper(PeopleShare<D>))]
impl<D: 'static + Clone + Eq + std::fmt::Debug> stef::State<'static> for People<D> {
    type Action = PeopleAction<D>;
    type Effect = Option<PeopleFx<D>>;

    pub fn add(&mut self, name: String, age: u8, data: D) -> Option<PeopleFx<D>> {
        self.0.push(Person {
            name: name.clone(),
            age,
            data: data.clone(),
        });
        Some(PeopleFx::SayHi(name, data))
    }

    pub fn clear(&mut self) -> Option<PeopleFx<D>> {
        self.0.clear();
        None
    }
}

#[test]
fn test_generics() {
    use stef::State;

    let mut p = People::<bool>(vec![]);

    let fx = p.transition(PeopleAction::Add("Mike".to_string(), 5, true));
    assert_eq!(fx, Some(PeopleFx::SayHi("Mike".to_string(), true)));

    p.add("Kayla".into(), 3, false).unwrap();

    assert_eq!(p.0.len(), 2);

    p.transition(PeopleAction::Clear);
    assert_eq!(p.0.len(), 0);

    let mut shared = PeopleShare::from(p);

    shared.add("Ryan".into(), 25, false);
    assert_eq!(shared.read(|s| s.0.len()), 1);
}
