use hdi::map_extern::ExternResult;
use holochain_zome_types::prelude::{GetValidationReceiptsInput, ValidationReceiptSet};
use crate::hdk::HDK;

pub fn get_validation_receipts(input: GetValidationReceiptsInput) -> ExternResult<Vec<ValidationReceiptSet>>
{
    HDK.with(|h| {
        h.borrow().get_validation_receipts(input)
    })
}
