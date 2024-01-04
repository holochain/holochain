use hdi::prelude::*;

#[hdk_extern]
fn genesis_self_check(_: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult> {
    let props = dna_info()?.modifiers.properties;

    // The default value is `()` which is serialized to `null`
    if props.bytes().len() == 1 {
        Ok(ValidateCallbackResult::Invalid("No properties".into()))
    } else {
        Ok(ValidateCallbackResult::Valid)
    }
}
