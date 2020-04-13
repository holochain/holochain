use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::RibosomeError;
use ring::rand;
use ring::rand::SecureRandom;
use std::sync::Arc;
use sx_zome_types::RandomBytesInput;
use sx_zome_types::RandomBytesOutput;

pub fn csprng_bytes(n: usize) -> Result<Vec<u8>, ring::error::Unspecified> {
    let rng = rand::SystemRandom::new();
    let mut bytes = Vec::with_capacity(n);
    bytes.resize(n, 0);
    rng.fill(&mut bytes)?;

    Ok(bytes)
}

pub fn random_bytes(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    input: RandomBytesInput,
) -> Result<RandomBytesOutput, RibosomeError> {
    Ok(RandomBytesOutput::new(csprng_bytes(input.into_inner())?))
}

#[cfg(test)]
pub mod wasm_test {
    use sx_zome_types::RandomBytesInput;
    use sx_zome_types::RandomBytesOutput;

    #[test]
    fn invoke_import_random_bytes_test() {
        let _: RandomBytesOutput =
            crate::call_test_ribosome!("imports", "random_bytes", RandomBytesInput::new(64));
    }
}
