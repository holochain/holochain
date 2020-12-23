use hdk3::prelude::*;
use holochain_test_wasm_common::TestBytes;
use holochain_test_wasm_common::TestInt;

#[hdk_entry(id = "post")]
struct Val(u32);

entry_defs![Val::entry_def()];

#[hdk_extern]
fn create_entry_multiple(n: TestInt) -> ExternResult<()> {
    for i in 0..n.0 {
        debug!(format!("{}", i));
        create_entry(&Val(i))?;
    }

    Ok(())
}

#[hdk_extern]
fn get_entry_multiple(n: TestInt) -> ExternResult<TestBytes> {
    let mut bytes = vec![];
    'test_loop: for i in 0..n.0 {
        match get(hash_entry(&Val(i))?, GetOptions::content())? {
            Some(element) => {
                match element.entry().to_app_option::<Val>()? {
                    Some(v) => bytes.append(&mut v.0.to_le_bytes().to_vec()),
                    // couldn't succeed to get so let's return what we have and let the test
                    // harness decide what that means
                    None => break 'test_loop,
                }
            }
            // as above
            None => break 'test_loop,
        }
    }

    Ok(TestBytes(bytes))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct TwoInt(pub u32, pub u32);

#[hdk_extern]
fn slow_fn(n: TwoInt) -> ExternResult<()> {
    for i in 0..n.1 {
        debug!("zome call: {} get call number: {}", n.0, i);
        get_links(hash_entry(&Val(i))?, None)?;
    }
    Ok(())
}
