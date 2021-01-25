//! @todo move all this out to the serialized bytes crate
use crate::prelude::*;
use holochain_serialized_bytes::prelude::*;
use rand::seq::SliceRandom;

#[derive(Clone, Copy)]
/// there are many different types of things that we could reasonably serialize in our examples
/// a list of things that we serialize iteratively (Predictable) or randomly (Unpredictable)
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

/// Serialization wrapper for bools
#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes)]
struct BoolWrap(bool);
/// Serialization wrapper for u32 (number)
#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes)]
struct U32Wrap(u32);
/// Serialzation wrapper for Strings
#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes)]
struct StringWrap(String);

fixturator!(
    SerializedBytes,
    { SerializedBytes::try_from(()).unwrap() },
    {
        // randomly select a thing to serialize
        let thing_to_serialize = THINGS_TO_SERIALIZE
            .to_vec()
            .choose(&mut crate::rng())
            .unwrap()
            .to_owned();

        // serialize a thing based on a delegated fixturator
        match thing_to_serialize {
            ThingsToSerialize::Unit =>
            {
                #[allow(clippy::unit_arg)]
                UnitFixturator::new(Unpredictable)
                    .next()
                    .unwrap()
                    .try_into()
                    .unwrap()
            }
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
        let mut index = get_fixt_index!();
        // iteratively select a thing to serialize
        let thing_to_serialize = THINGS_TO_SERIALIZE
            .to_vec()
            .into_iter()
            .cycle()
            .nth(index)
            .unwrap();

        // serialize a thing based on a delegated fixturator
        let ret: SerializedBytes = match thing_to_serialize {
            ThingsToSerialize::Unit =>
            {
                #[allow(clippy::unit_arg)]
                UnitFixturator::new_indexed(Predictable, index)
                    .next()
                    .unwrap()
                    .try_into()
                    .unwrap()
            }
            ThingsToSerialize::Bool => BoolWrap(
                BoolFixturator::new_indexed(Predictable, index)
                    .next()
                    .unwrap(),
            )
            .try_into()
            .unwrap(),
            ThingsToSerialize::Number => U32Wrap(
                U32Fixturator::new_indexed(Predictable, index)
                    .next()
                    .unwrap(),
            )
            .try_into()
            .unwrap(),
            ThingsToSerialize::String => StringWrap(
                StringFixturator::new_indexed(Predictable, index)
                    .next()
                    .unwrap(),
            )
            .try_into()
            .unwrap(),
        };

        index += 1;
        set_fixt_index!(index);
        ret
    }
);
