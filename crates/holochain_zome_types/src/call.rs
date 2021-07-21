use crate::capability::CapSecret;
use crate::cell::CellId;
use crate::zome::FunctionName;
use crate::zome::ZomeName;
use crate::ExternIO;
use holo_hash::AgentPubKey;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Call {
    pub to_cells: Vec<Option<CellId>>,
    pub zome_name: ZomeName,
    pub fn_name: FunctionName,
    pub cap: Option<CapSecret>,
    pub payload: ExternIO,
    pub provenance: AgentPubKey,
}

impl Call {
    pub fn new(
        to_cells: Vec<Option<CellId>>,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
        payload: ExternIO,
        provenance: AgentPubKey,
    ) -> Self {
        Self {
            to_cells,
            zome_name,
            fn_name,
            cap,
            payload,
            provenance,
        }
    }
}
