//! This crate optionally exposes the HDI and HDK.
//! The main rationale for this is to have guaranateed compatibility.

#[cfg(feature = "hdk")]
pub use hdk;

#[cfg(feature = "hdi")]
pub use hdi;

#[cfg(test)]
mod tests {
    use std::{fmt::Debug, path::Path};

    use cargo::{
        core::{compiler::CompileMode, Manifest, Package, Workspace},
        ops::{CompileOptions, TestOptions},
        CliError,
    };

    #[test]
    fn export_none() {
        let compile_thread = std::thread::spawn(|| {
            let config = cargo::Config::default().unwrap();

            let workspace = Workspace::new(
                std::env::current_dir()
                    .unwrap()
                    .as_path()
                    .join("tests/none/Cargo.toml")
                    .as_path(),
                &config,
            )
            .unwrap();

            let options: TestOptions = TestOptions {
                compile_opts: CompileOptions::new(&config, CompileMode::Test).unwrap(),
                no_run: false,
                no_fail_fast: false,
            };

            cargo::ops::compile(
                &workspace,
                &CompileOptions::new(&config, CompileMode::Test).unwrap(),
            )
            .map_err(|e| e.to_string())
        });

        let error = match compile_thread.join() {
            Ok(_) => panic!("succeeded"),
            Err(e) => panic!("{e:?}"),
        };

        Ok(error)

        // let error = error.to_string();

        // let error = cargo::ops::run_tests(&workspace, &options, &test_args).unwrap_err();

        // assert!(
        //     format!("{error:?}").contains("note: the item is gated behind the `hdi` feature"),
        //     "{error:#?}"
        // )
    }

    #[test]
    fn export_hdi() {}

    #[test]
    fn export_hdk() {}

    #[test]
    fn export_all() {}
}
