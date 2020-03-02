#![feature(backtrace)]
use error::WorkspaceResult;
use shrinkwraprs::Shrinkwrap;

pub mod buffer;
pub mod db;
pub mod env;
pub mod error;

// NB: would be nice to put this under cfg(test), but then it's not visible from other crates,
// since cfg(test) only applies to the crate in which you run tests
pub mod test_utils;

pub type Reader<'env> = rkv::Reader<'env>;
pub type Writer<'env> = rkv::Writer<'env>;
pub type SingleStore = rkv::SingleStore;
pub type IntegerStore = rkv::IntegerStore<u32>;
pub type MultiStore = rkv::MultiStore;

// TODO: remove ASAP, once we know how to actually create an env and get databases
#[derive(Shrinkwrap)]
pub struct RkvEnv(rkv::Rkv);

impl RkvEnv {
    pub fn read(&self) -> WorkspaceResult<Reader> {
        Ok(self.0.read()?)
    }

    pub fn write(&self) -> WorkspaceResult<Writer> {
        Ok(self.0.write()?)
    }
}

pub struct Env(rkv::Rkv);

impl Env {
    pub fn read(&self) -> WorkspaceResult<Reader> {
        Ok(self.0.read()?)
    }

    pub fn write(&self) -> WorkspaceResult<Writer> {
        Ok(self.0.write()?)
    }
}
