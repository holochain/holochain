// TODO: wrap in newtype so that better errors can be created
pub type Writer<'env> = rkv::Writer<'env>;

pub type SingleStore = rkv::SingleStore;
pub type IntegerStore = rkv::IntegerStore<u32>;
pub type MultiStore = rkv::MultiStore;
