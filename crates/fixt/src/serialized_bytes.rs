//! @todo move all this out to the serialized bytes crate
use crate::prelude::*;
use holochain_serialized_bytes::prelude::*;
use rand::seq::SliceRandom;

#[derive(Clone, Copy)]
pub enum ThingsToSerialize {
    Unit,
    Bool,
    Number,
    String,
}

pub const THINGS_TO_SERIALIZE: [ThingsToSerialize; 4] = [
    ThingsToSerialize::Unit,
    ThingsToSerialize::Bool,
    ThingsToSerialize::Number,
    ThingsToSerialize::String,
];

#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
struct BoolWrap(bool);
#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
struct U32Wrap(u32);
#[derive(serde::Serialize, serde::Deserialize, SerializedBytes)]
struct StringWrap(String);

fixturator!(
    SerializedBytes,
    { SerializedBytes::try_from(()).unwrap() },
    {
        let thing_to_serialize = THINGS_TO_SERIALIZE
            .to_vec()
            .choose(&mut rand::thread_rng())
            .unwrap()
            .to_owned();

        match thing_to_serialize {
            ThingsToSerialize::Unit => UnitFixturator::new(Unpredictable)
                .next()
                .unwrap()
                .try_into()
                .unwrap(),
            ThingsToSerialize::Bool => BoolWrap(BoolFixturator::new(Unpredictable).next().unwrap())
                .try_into()
                .unwrap(),
            ThingsToSerialize::Number => U32Wrap(U32Fixturator::new(Unpredictable).next().unwrap())
                .try_into()
                .unwrap(),
            ThingsToSerialize::String => {
                StringWrap(StringFixturator::new(Unpredictable).next().unwrap())
                    .try_into()
                    .unwrap()
            }
        }
    },
    {
        let thing_to_serialize = THINGS_TO_SERIALIZE
            .to_vec()
            .into_iter()
            .cycle()
            .nth(self.0.index)
            .unwrap();

        let ret: SerializedBytes = match thing_to_serialize {
            ThingsToSerialize::Unit => UnitFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap()
                .try_into()
                .unwrap(),
            ThingsToSerialize::Bool => BoolWrap(
                BoolFixturator::new_indexed(Predictable, self.0.index)
                    .next()
                    .unwrap(),
            )
            .try_into()
            .unwrap(),
            ThingsToSerialize::Number => U32Wrap(
                U32Fixturator::new_indexed(Predictable, self.0.index)
                    .next()
                    .unwrap(),
            )
            .try_into()
            .unwrap(),
            ThingsToSerialize::String => StringWrap(
                StringFixturator::new_indexed(Predictable, self.0.index)
                    .next()
                    .unwrap(),
            )
            .try_into()
            .unwrap(),
        };

        self.0.index = self.0.index + 1;
        ret
    }
);
