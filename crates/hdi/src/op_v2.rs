//! v2 of [`OpHelper`](crate::op::OpHelper): flattens a v2
//! [`Op`](holochain_integrity_types::dht_v2::Op) into the v2
//! [`FlatOp`](crate::flat_op_v2::FlatOp). Transitional staging module; promoted
//! to replace `op`'s helper in the legacy-deletion phase.

use crate::prelude::*;

/// Conversion from a v2 [`Op`](holochain_integrity_types::dht_v2::Op) to a v2
/// [`FlatOp`](crate::flat_op_v2::FlatOp), for use in the validate callback.
pub trait OpHelper {
    /// Convert without consuming, cloning the required internal data.
    fn flattened<ET, LT>(&self) -> Result<crate::flat_op_v2::FlatOp<ET, LT>, WasmError>
    where
        ET: EntryTypesHelper + UnitEnum,
        <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
        LT: LinkTypesHelper,
        WasmError: From<<ET as EntryTypesHelper>::Error>,
        WasmError: From<<LT as LinkTypesHelper>::Error>;
}
