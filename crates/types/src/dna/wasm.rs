//! sx_types::dna::wasm is a module for managing webassembly code
//!  - within the in-memory dna struct
//!  - and serialized to json
use crate::error::SkunkError;
use backtrace::Backtrace;

use base64;
use log::*;
use serde::{
    self,
    de::{Deserializer, Visitor},
    ser::Serializer,
    Deserialize, Serialize,
};
use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    sync::{Arc, RwLock},
};
// use wasmi::Module;
// TODO: dummy shim for wasm, will be replaced by wasmer soon
pub struct Module;

/// Wrapper around wasmi::Module since it does not implement Clone, Debug, PartialEq, Eq,
/// which are all needed to add it to the DnaWasm below, and hence to the state.
#[derive(Clone)]
pub struct ModuleArc(Arc<Module>);
impl ModuleArc {
    pub fn new(module: Module) -> Self {
        ModuleArc(Arc::new(module))
    }
}
impl PartialEq for ModuleArc {
    fn eq(&self, _other: &ModuleArc) -> bool {
        //*self == *other
        false
    }
}
impl Eq for ModuleArc {}
impl Deref for ModuleArc {
    type Target = Arc<Module>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl fmt::Debug for ModuleArc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ModuleMutex")
    }
}

/// Private helper for converting binary WebAssembly into base64 serialized string.
fn _vec_u8_to_b64_str<S>(data: &Arc<Vec<u8>>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let b64 = base64::encode(data.as_ref());
    s.serialize_str(&b64)
}

/// Private helper for converting base64 string into binary WebAssembly.
fn _b64_str_to_vec_u8<'de, D>(d: D) -> Result<Arc<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    /// visitor struct needed for serde deserialization
    struct Z;

    impl<'de> Visitor<'de> for Z {
        type Value = Vec<u8>;

        /// we only want to accept strings
        fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
            formatter.write_str("string")
        }

        /// if we get a string, try to base64 decode into binary
        fn visit_str<E>(self, value: &str) -> Result<Vec<u8>, E>
        where
            E: serde::de::Error,
        {
            match base64::decode(value) {
                Ok(v) => Ok(v),
                Err(e) => Err(serde::de::Error::custom(e)),
            }
        }
    }

    Ok(Arc::new(d.deserialize_any(Z)?))
}

/// Represents web assembly code.
#[derive(Serialize, Deserialize, Clone)]
pub struct DnaWasm {
    /// The actual binary WebAssembly bytecode goes here.
    #[serde(
        serialize_with = "_vec_u8_to_b64_str",
        deserialize_with = "_b64_str_to_vec_u8"
    )]
    pub code: Arc<Vec<u8>>,

    /// This is a transient parsed representation of the binary code.
    /// This gets only create once from the code and then cached inside this RwLock
    /// because creation of these WASMi modules from bytes is expensive.
    #[serde(skip, default = "empty_module")]
    module: Arc<RwLock<Option<ModuleArc>>>,
}

impl DnaWasm {
    /// Provide basic placeholder for wasm entries in dna structs, used for testing only.
    pub fn new_invalid() -> Self {
        debug!(
            "DnaWasm::new_invalid() called from:\n{:?}",
            Backtrace::new()
        );
        DnaWasm {
            code: Arc::new(vec![]),
            module: empty_module(),
        }
    }
}

fn empty_module() -> Arc<RwLock<Option<ModuleArc>>> {
    Arc::new(RwLock::new(None))
}

impl fmt::Debug for DnaWasm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<<<DNA WASM CODE>>>")
    }
}

impl PartialEq for DnaWasm {
    fn eq(&self, other: &DnaWasm) -> bool {
        self.code == other.code
    }
}

impl Hash for DnaWasm {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.code.hash(state);
    }
}

impl DnaWasm {
    /// Creates a new instance from given WASM binary
    pub fn from_bytes(wasm: Vec<u8>) -> Self {
        DnaWasm {
            code: Arc::new(wasm),
            module: empty_module(),
        }
    }

    /// This returns a parsed WASMi representation of the code, ready to be
    /// run in a WASMi ModuleInstance.
    /// The first call will create the module from the binary.
    pub fn get_wasm_module(&self) -> Result<ModuleArc, SkunkError> {
        if self.module.read().unwrap().is_none() {
            self.create_module()?;
        }

        Ok(self.module.read().unwrap().as_ref().unwrap().clone())
    }

    fn create_module(&self) -> Result<(), SkunkError> {
        unimplemented!()
        // let module = wasmi::Module::from_buffer(&*self.code).map_err(|e| {
        //     debug!(
        //         "DnaWasm could not create a wasmi::Module from code bytes! Error: {:?}",
        //         e
        //     );
        //     debug!("Unparsable bytes: {:?}", *self.code);
        //     SkunkError::Todo(e.into())
        // })?;
        // let module_arc = ModuleArc::new(module);
        // let mut lock = self.module.write().unwrap();
        // *lock = Some(module_arc);
        // Ok(())
    }
}
