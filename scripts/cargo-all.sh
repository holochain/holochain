
if [ $# -gt 1 ]
then
    cargo $1 --manifest-path Cargo.toml ${@:2}
    cargo $1 --manifest-path crates/release-automation/Cargo.toml ${@:2}
    cargo $1 --manifest-path crates/test_utils/wasm/wasm_workspace/Cargo.toml ${@:2}
else
    cargo $1 --manifest-path Cargo.toml
    cargo $1 --manifest-path crates/release-automation/Cargo.toml
    cargo $1 --manifest-path crates/test_utils/wasm/wasm_workspace/Cargo.toml
fi