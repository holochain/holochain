struct Elevator;

#[derive(Debug, PartialEq, Eq, derive_more::From)]
pub enum ElevatorEffect {
    Command(String),
    Foo(u32),
}

#[stef::state]
impl stef::State<'static> for Elevator {
    type Action = ElevatorTransition;
    type Effect = ElevatorEffect;

    #[stef::state(matches(ElevatorEffect::Command(x) => x))]
    fn go(&mut self, action: String) -> String {
        format!("{}!", action)
    }

    #[stef::state(matches(ElevatorEffect::Foo(x) => x))]
    fn foo(&mut self, (_a, _b): (String, String)) -> u32 {
        todo!()
    }
}

#[derive(Debug, PartialEq, Eq, derive_more::From)]
pub enum BananaEffect {
    Grow,
    Rot,
}

struct Banana;

#[stef::state]
impl stef::State<'static> for Banana {
    type Action = BananaAction;
    type Effect = Option<BananaEffect>;

    fn peel(&mut self, (): ()) -> Option<BananaEffect> {
        Some(BananaEffect::Rot)
    }

    fn foo(&mut self, (_a, _b): (String, String)) -> Option<BananaEffect> {
        None
    }
}

fn main() {
    use stef::State;
    {
        let mut e = Elevator;
        let plain = e.transition(ElevatorTransition::Go("up".into()));
        let sugar = e.go("up".to_string());
        let sugar: ElevatorEffect = sugar.into();

        assert_eq!(plain, sugar);

        match plain {
            ElevatorEffect::Command(c) => assert_eq!(c, "up!".to_string()),
            _ => unreachable!(),
        }
    }

    {
        let mut b = Banana;
        assert_eq!(b.peel(()), Some(BananaEffect::Rot));
    }
}
