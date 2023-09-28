use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum MyEffect {
    NotifySomeone(u32),
}

#[derive(Debug, Clone)]
pub enum MyAction {
    Add(u32),
}

pub struct MyState {
    state: Mutex<u32>,
    action_tee: Option<Arc<Mutex<Vec<MyAction>>>>,
    effect_tee: Option<Arc<Mutex<Vec<MyEffect>>>>,
}

impl MyState {
    pub fn new(
        init: u32,
        action_tee: Option<Arc<Mutex<Vec<MyAction>>>>,
        effect_tee: Option<Arc<Mutex<Vec<MyEffect>>>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(init),
            action_tee,
            effect_tee,
        })
    }

    pub fn transition(&self, action: MyAction) -> MyEffect {
        if let Some(action_tee) = self.action_tee.as_ref() {
            action_tee.lock().unwrap().push(action.clone());
        }
        let effect = match action {
            MyAction::Add(add) => {
                let mut lock = self.state.lock().unwrap();
                *lock += add;
                MyEffect::NotifySomeone(*lock)
            }
        };
        if let Some(effect_tee) = self.effect_tee.as_ref() {
            effect_tee.lock().unwrap().push(effect.clone());
        }
        effect
    }
}

fn main() {
    let action_tee = Arc::new(Mutex::new(Vec::new()));
    let effect_tee = Arc::new(Mutex::new(Vec::new()));

    let state = MyState::new(42, Some(action_tee.clone()), Some(effect_tee.clone()));
    let _ = state.transition(MyAction::Add(2));
    let _ = state.transition(MyAction::Add(3));

    println!("actions: {:#?}", action_tee.lock().unwrap());
    println!("effects: {:#?}", effect_tee.lock().unwrap());
}
