use std::sync::{atomic::AtomicBool, Arc};

use stef::*;

use either::Either;

/// A state machine
#[derive(Debug, Default)]
struct TrafficLight(u8);

/// A representation of the TrafficLight state
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum TrafficLightColor {
    Red,
    Amber,
    Green,
}
use TrafficLightColor::*;

/// Effects triggered by state transitions
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum TrafficLightEffect {
    StartThatBlinkyBlueLight,
    StopThatBlinkyBlueLight,
}
use TrafficLightEffect::*;

impl TrafficLight {
    fn color(&self) -> TrafficLightColor {
        match self.0 {
            0 => Green,
            1 => Amber,
            2 => Red,
            _ => unreachable!(),
        }
    }
}

impl State for TrafficLight {
    type Action = ();
    type Effect = Option<TrafficLightEffect>;

    fn transition(&mut self, _: Self::Action) -> Self::Effect {
        // Cycle through green, amber, red, and repeat
        self.0 += 1;
        self.0 %= 3;
        match self.color() {
            Red => Some(StartThatBlinkyBlueLight),
            Green => Some(StopThatBlinkyBlueLight),
            Amber => None,
        }
    }
}

/// Another state machine
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
struct WalkSign {
    capacity: u8,
    countdown: u8,
}

/// Representation of state
#[derive(PartialEq, Eq, Debug)]
enum WalkSignIcon {
    Stop,
    Go(u8),
}
use WalkSignIcon::*;

#[must_use]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum WalkSignEffect {
    StartVoiceMessage,
    StopVoiceMessage,
}
use WalkSignEffect::*;

impl WalkSign {
    fn new(capacity: u8) -> Self {
        Self {
            capacity,
            countdown: capacity,
        }
    }
    fn icon(&self) -> WalkSignIcon {
        match self.countdown {
            0 => Stop,
            n => Go(n),
        }
    }
}

impl ParamState for WalkSign {
    type State = u8;
    type Params = u8;
    type Action = ();
    type Effect = Option<WalkSignEffect>;

    fn initial(capacity: u8) -> Self {
        WalkSign::new(capacity)
    }

    fn partition(&mut self) -> (&mut Self::State, &Self::Params) {
        (&mut self.countdown, &self.capacity)
    }

    fn update(
        countdown: &mut Self::State,
        capacity: &Self::Params,
        _action: Self::Action,
    ) -> Self::Effect {
        if *countdown == 0 {
            *countdown = *capacity;
        } else {
            *countdown -= 1;
        }
        match countdown {
            0 => Some(StartVoiceMessage),
            n if *n == *capacity => Some(StopVoiceMessage),
            _ => None,
        }
    }
}

#[test]
fn share() {
    let light = Share::new(TrafficLight(0));

    assert_eq!(light.read(TrafficLight::color), Green);
    assert_eq!(light.transition(()), None);
    assert_eq!(light.read(TrafficLight::color), Amber);
    assert_eq!(light.transition(()), Some(StartThatBlinkyBlueLight));
    assert_eq!(light.read(TrafficLight::color), Red);
    assert_eq!(light.transition(()), Some(StopThatBlinkyBlueLight));
    assert_eq!(light.read(TrafficLight::color), Green);
}

#[test]
fn share_accumulated() {
    let mut light = TrafficLight(0).shared().store_effects(10);

    assert_eq!(light.read(TrafficLight::color), Green);

    let () = light.transition(());
    assert_eq!(light.read(TrafficLight::color), Amber);

    light.transition(());
    assert_eq!(light.read(TrafficLight::color), Red);

    light.transition(());
    assert_eq!(light.read(TrafficLight::color), Green);

    let expected = vec![
        None,
        Some(StartThatBlinkyBlueLight),
        Some(StopThatBlinkyBlueLight),
    ];

    assert_eq!(light.effects().as_slices().0, expected.as_slice());
    assert_eq!(light.drain_effects(), expected);
    assert_eq!(light.effects(), &[]);
}

#[test]
fn share_runner() {
    let blinking = Arc::new(AtomicBool::new(false));
    let blinky = blinking.clone();

    let mut share = TrafficLight(0).shared().run_effects(move |eff| match eff {
        Some(StartThatBlinkyBlueLight) => {
            assert!(!blinking.swap(true, std::sync::atomic::Ordering::Relaxed));
            true
        }
        Some(StopThatBlinkyBlueLight) => {
            assert!(blinking.swap(false, std::sync::atomic::Ordering::Relaxed));
            true
        }
        None => false,
    });

    assert_eq!(share.read(TrafficLight::color), Green);

    assert!(!share.transition(()));
    assert!(!blinky.load(std::sync::atomic::Ordering::Relaxed));
    assert_eq!(share.read(TrafficLight::color), Amber);

    assert!(share.transition(()));
    assert!(blinky.load(std::sync::atomic::Ordering::Relaxed));
    assert_eq!(share.read(TrafficLight::color), Red);

    assert!(share.transition(()));
    assert!(!blinky.load(std::sync::atomic::Ordering::Relaxed));
    assert_eq!(share.read(TrafficLight::color), Green);
}

#[test]
fn composition() {
    #[derive(Debug)]
    struct Intersection {
        light: Share<TrafficLight>,
        walk: Share<WalkSign>,
    }

    impl State for Intersection {
        type Action = ();
        type Effect = Vec<Either<TrafficLightEffect, WalkSignEffect>>;

        fn transition(&mut self, (): Self::Action) -> Self::Effect {
            let (m, e) = self.walk.transition_with((), |w| {
                if [0, 3, w.capacity].contains(&w.countdown) {
                    self.light.transition(())
                } else {
                    None
                }
            });
            [m.map(Either::Left), e.map(Either::Right)]
                .into_iter()
                .flatten()
                .collect()
        }
    }

    let light = Share::new(TrafficLight(0));
    let walk = Share::new(WalkSign::new(10));
    let mut s = StoreEffects::new(Intersection { light, walk }, 10);

    for _ in 0..6 {
        s.transition(());
    }
    assert_eq!(s.walk.read(WalkSign::icon), Go(4));
    assert_eq!(s.light.read(TrafficLight::color), Green);

    s.transition(());
    assert_eq!(s.walk.read(WalkSign::icon), Go(3));
    assert_eq!(s.light.read(TrafficLight::color), Amber);

    s.transition(());
    s.transition(());
    assert_eq!(s.light.read(TrafficLight::color), Amber);
    s.transition(());
    assert_eq!(s.walk.read(WalkSign::icon), Stop);
    assert_eq!(s.light.read(TrafficLight::color), Red);

    s.transition(());
    assert_eq!(s.walk.read(WalkSign::icon), Go(10));
    assert_eq!(s.light.read(TrafficLight::color), Green);

    use Either::*;

    let fx: Vec<_> = s.drain_effects().into_iter().flatten().collect();
    assert_eq!(
        fx,
        vec![
            Left(StartThatBlinkyBlueLight),
            Right(StartVoiceMessage),
            Left(StopThatBlinkyBlueLight),
            Right(StopVoiceMessage)
        ]
    )
}
