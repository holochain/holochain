use stef::{ParamState, State};

struct Foo {
    state: FooState,
    multiplier: u32,
}

#[derive(Default)]
struct FooState(u32);

impl ParamState<'static> for Foo {
    type State = FooState;
    type Params = u32;
    type Action = u32;
    type Effect = ();

    fn initial(params: Self::Params) -> Self {
        Self {
            state: Default::default(),
            multiplier: params,
        }
    }

    fn partition(&mut self) -> (&mut Self::State, &Self::Params) {
        (&mut self.state, &self.multiplier)
    }

    fn update(state: &mut Self::State, mul: &Self::Params, add: Self::Action) -> Self::Effect {
        state.0 += add * mul;
    }
}

fn main() {
    let mut foo = Foo {
        state: FooState(1),
        multiplier: 2,
    };

    foo.transition(3);

    assert_eq!(foo.state.0, 7)
}
