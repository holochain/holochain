#[allow(dead_code)]
struct Person<D> {
    name: String,
    age: u8,
    data: D,
}

#[derive(Default)]
struct People<D>(Vec<Person<D>>);

#[derive(PartialEq, Eq, Debug)]
enum PeopleFx<D> {
    SayHi(String, D),
}

#[stef_derive::state]
impl<D: 'static + Clone + Eq> stef::State for People<D> {
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
}
