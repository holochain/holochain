use std::sync::{Arc, Mutex};

use stef::State;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MyEffect {
    NotifySomeone(u32),
}

#[derive(Debug, Clone)]
pub enum MyAction {
    Add(u32),
}

pub struct MyState(u32);

impl stef::State<'static> for MyState {
    type Action = MyAction;
    type Effect = MyEffect;

    fn transition(&mut self, action: Self::Action) -> Self::Effect {
        match action {
            MyAction::Add(add) => {
                self.0 += add;
                MyEffect::NotifySomeone(self.0)
            }
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
