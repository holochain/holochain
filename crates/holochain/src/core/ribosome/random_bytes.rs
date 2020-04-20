use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::RibosomeError;
use ring::rand;
use ring::rand::SecureRandom;
use std::sync::Arc;
use sx_zome_types::RandomBytesInput;
use sx_zome_types::RandomBytesOutput;

/// generate n crypto secure random bytes from the system
/// may also be called directly by things other than the `random_bytes` ribosome invocation
pub fn csprng_bytes(n: usize) -> Result<Vec<u8>, ring::error::Unspecified> {
    let rng = rand::SystemRandom::new();
    let mut bytes = vec![0; n];
    bytes.resize(n, 0);
    rng.fill(&mut bytes)?;

    Ok(bytes)
}

/// thin wrapper around csprng to expose it to the ribosome
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
    use crate::core::ribosome::random_bytes::csprng_bytes;

    #[test]
    fn invoke_import_random_bytes_test() {
        // generate random bytes through a ribosome invocation
        let invoke_closure = |i| {
            let output: RandomBytesOutput = crate::call_test_ribosome!("imports", "random_bytes", RandomBytesInput::new(i));
            output.into_inner()
        };
        // generate random bytes through a direct function call
        let direct_closure = |i| {
            csprng_bytes(i).unwrap()
        };

        let i = 64;
        macro_rules! do_test {
            ( $( $c:ident ),* ) => {
                $(
                    let o = $c(i);
                    // number of bytes needs to match input
                    assert!(o.len() == i);
                    // bytes need to be filled correctly (not left as default 0 bytes)
                    assert_ne!(vec![0; i], o);
                )*
            };
        }
        do_test!(invoke_closure, direct_closure);

    }
}
