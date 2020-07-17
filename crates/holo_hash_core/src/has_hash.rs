use crate::{HashType, HoloHashImpl};

pub trait HasHash<T: HashType> {
    fn hash(&self) -> &HoloHashImpl<T>;
    fn into_hash(self) -> HoloHashImpl<T>;
}
