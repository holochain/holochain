//! Helpers for unit tests

use crate::env::{Environment, EnvironmentKind};
use sx_types::test_utils::fake_cell_id;
use tempdir::TempDir;

/// Create an [TestEnvironment] of [EnvironmentKind::Cell], backed by a temp directory
pub fn test_cell_env() -> TestEnvironment {
    let cell_id = fake_cell_id(&nanoid::nanoid!());
    test_env(EnvironmentKind::Cell(cell_id))
}

/// Create an [TestEnvironment] of [EnvironmentKind::Conductor], backed by a temp directory
pub fn test_conductor_env() -> TestEnvironment {
    test_env(EnvironmentKind::Conductor)
}

fn test_env(kind: EnvironmentKind) -> TestEnvironment {
    let tmpdir = TempDir::new("holochain-test-environments").unwrap();
    // TODO: Wrap Environment along with the TempDir so that it lives longer
    Environment::new(tmpdir.path(), kind).expect("Couldn't create test LMDB environment")
}

type TestEnvironment = Environment;

// FIXME: Currently the test environments using TempDirs above immediately
// delete the temp dirs after installation. If we ever have cases where we
// want to flush to disk, this will probably fail. In that case we want to
// use something like this, which owns the TempDir so it lives long enough
//
// /// A wrapper around an Environment which includes a reference to a TempDir,
// /// so that when the TestEnvironment goes out of scope, the tempdir is deleted
// /// from the filesystem
// #[derive(Shrinkwrap)]
// pub struct TestEnvironment(#[shrinkwrap(main_field)] Environment, TempDir);
