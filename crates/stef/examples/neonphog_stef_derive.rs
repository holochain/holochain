use std::sync::{Arc, Mutex};

use stef::State;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MyEffect {
    NotifySomeone(u32),
}

pub struct MyState(u32);

#[stef::state]
impl stef::State<'static> for MyState {
    type Action = MyAction;
    type Effect = MyEffect;

    #[stef::state(
        matches(
            MyEffect::NotifySomeone(x) <=> x
        )
    )]
    fn add(&mut self, amount: u32) -> u32 {
        self.0 += amount;
        self.0
    }
}

/// NOTE: we could easily derive Clone for the generated action type
/// or add an attr to allow the option to do so or not
impl Clone for MyAction {
    fn clone(&self) -> Self {
        match self {
            Self::Add(a) => Self::Add(a.clone()),
        }
    }
}

pub struct MyData {
    state: Mutex<MyState>,
    action_tee: Option<Arc<Mutex<Vec<MyAction>>>>,
    effect_tee: Option<Arc<Mutex<Vec<MyEffect>>>>,
}

impl MyData {
    pub fn new(
        init: u32,
        action_tee: Option<Arc<Mutex<Vec<MyAction>>>>,
        effect_tee: Option<Arc<Mutex<Vec<MyEffect>>>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(MyState(init)),
            action_tee,
            effect_tee,
        })
    }

    pub fn transition(&self, action: MyAction) -> MyEffect {
        if let Some(action_tee) = self.action_tee.as_ref() {
            action_tee.lock().unwrap().push(action.clone());
        }
        let effect = self.state.lock().unwrap().transition(action);
        if let Some(effect_tee) = self.effect_tee.as_ref() {
            effect_tee.lock().unwrap().push(effect.clone());
        }
        effect
    }
}

fn main() {
    let action_tee = Arc::new(Mutex::new(Vec::new()));
    let effect_tee = Arc::new(Mutex::new(Vec::new()));

    let state = MyData::new(42, Some(action_tee.clone()), Some(effect_tee.clone()));
    let _ = state.transition(MyAction::Add(2));
    let _ = state.transition(MyAction::Add(3));

    println!("actions: {:#?}", action_tee.lock().unwrap());
    println!("effects: {:#?}", effect_tee.lock().unwrap());
}
