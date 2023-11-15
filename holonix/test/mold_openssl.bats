#!/usr/bin/env bats

setup() {
  BATS_TMPDIR="$(mktemp --directory --dry-run)"
  cp -LRv ./test/mold_openssl "${BATS_TMPDIR}"
  find "${BATS_TMPDIR}/." -exec chmod +w {} \;
  mkdir "${BATS_TMPDIR}"/.cargo
  cp -LRv ${CARGO_VENDOR_DIR:?}/config.toml "${BATS_TMPDIR}"/.cargo/
  cd "${BATS_TMPDIR}"
}

teardown() {
  cd ..
  rm -rf "${BATS_TMPDIR:?}"
}

@test "test binary runs successfully" {
    cargo test --offline --locked
}
