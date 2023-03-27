mod version_info {
    use chrono::{offset::Utc, DateTime};
    use serde::Serialize;
    use std::{process::Command, time::SystemTime};

    #[derive(Serialize, Debug)]
    struct BuildInfo {
        git_info: Option<GitInfo>,
        cargo_pkg_version: &'static str,
        hdk_version_req: &'static str,
        hdi_version_req: &'static str,

        timestamp: DateTime<Utc>,
        hostname: String,

        host: String,
        target: String,
        rustc_version: String,
        rustflags: String,
        profile: String,
    }
    #[derive(Serialize, Debug)]
    struct GitInfo {
        rev: String,
        dirty: bool,
    }

    impl GitInfo {
        fn maybe_retrieve() -> Option<Self> {
            let git_available = Command::new("git")
                .arg("status")
                .output()
                .map(|output| output.status.code().unwrap_or(1))
                .unwrap_or(1)
                == 0;

            if !git_available {
                None
            } else {
                let git_rev = String::from_utf8_lossy(
                    &Command::new("git")
                        .arg("rev-parse")
                        .arg("HEAD")
                        .output()
                        .unwrap()
                        .stdout,
                )
                .trim()
                .to_string();

                let git_dirty = Command::new("git")
                    .arg("diff")
                    .arg("--quiet")
                    .arg("--exit-code")
                    .spawn()
                    .unwrap()
                    .wait()
                    .unwrap()
                    .code()
                    .unwrap()
                    != 0;

                Some(Self {
                    rev: git_rev,
                    dirty: git_dirty,
                })
            }
        }
    }

    impl BuildInfo {
        fn retrieve() -> Self {
            let rustc_version = Command::new(option_env!("RUSTC").unwrap_or("rustc"))
                .arg("--version")
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .unwrap_or_default();

            let hostname = hostname::get()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            BuildInfo {
                cargo_pkg_version: env!("CARGO_PKG_VERSION"),
                git_info: GitInfo::maybe_retrieve(),
                hdk_version_req: hdk::HDK_VERSION,
                hdi_version_req: hdk::HDI_VERSION,

                timestamp: SystemTime::now().into(),
                hostname,

                host: std::env::var("HOST").unwrap_or_default(),
                target: std::env::var("TARGET").unwrap_or_default(),
                rustc_version,
                rustflags: std::env::var("RUSTFLAGS")
                    .ok()
                    .or_else(|| option_env!("RUSTFLAGS").map(|s| s.to_string()))
                    .unwrap_or_default(),
                profile: std::env::var("PROFILE").unwrap_or_default(),
            }
        }

        fn as_json_string(&self) -> String {
            serde_json::to_string(&self).unwrap()
        }
    }

    /// This will be used populate the VERSION_INFO environment variable,
    /// which will be displayed as JSON when `holochain --version-info` is called.
    pub(crate) fn populate_env() {
        let json = BuildInfo::retrieve().as_json_string();
        println!("cargo:rustc-env=BUILD_INFO={}", json);

        // incase you want to debug the output:
        //println!("cargo:warning={}", json);
    }
}

fn main() {
    // inwelling::register();
    version_info::populate_env();
}
