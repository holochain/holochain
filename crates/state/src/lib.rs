#![feature(backtrace)]
use error::WorkspaceResult;
use shrinkwraprs::Shrinkwrap;

pub mod buffer;
pub mod db;
pub mod env;
pub mod error;
pub mod workspace;

pub type Reader<'env> = rkv::Reader<'env>;
pub type Writer<'env> = rkv::Writer<'env>;

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
