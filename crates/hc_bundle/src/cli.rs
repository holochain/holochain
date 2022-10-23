#![forbid(missing_docs)]
//! Binary `hc-dna` command executable.

use holochain_types::prelude::{AppManifest, DnaManifest, ValidatedDnaManifest};
use holochain_types::web_app::WebAppManifest;
use holochain_util::ffs;
use mr_bundle::{Location, Manifest};
use std::path::Path;
use std::path::PathBuf;
use structopt::StructOpt;

use crate::error::HcBundleResult;

/// The file extension to use for DNA bundles
pub const DNA_BUNDLE_EXT: &str = "dna";

/// The file extension to use for hApp bundles
pub const APP_BUNDLE_EXT: &str = "happ";

/// The file extension to use for Web-hApp bundles
pub const WEB_APP_BUNDLE_EXT: &str = "webhapp";

/// Work with Holochain DNA bundles
#[derive(Debug, StructOpt)]
pub enum HcDnaBundle {
    /// Create a new, empty Holochain DNA bundle working directory and create a new
    /// sample `dna.yaml` manifest inside.
    /// .
    Init {
        /// The path to create the working directory
        path: PathBuf,
    },

    /// Pack into the `[name].dna` bundle according to the `dna.yaml` manifest,
    /// found inside the working directory. The `[name]` is taken from the `name`
    /// property of the manifest file.
    ///
    /// e.g.:
    ///
    /// $ hc dna pack ./some/directory/foo
    ///
    /// creates a file `./some/directory/foo/[name].dna`, based on
    /// `./some/directory/foo/dna.yaml`
    Pack {
        /// The path to the working directory containing a `dna.yaml` manifest
        path: std::path::PathBuf,

        /// Specify the output path for the packed bundle file
        ///
        /// If not specified, the `[name].dna` bundle will be placed inside the
        /// provided working directory.
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,
    },

    /// Unpack parts of the `.dna` bundle file into a specific directory.
    ///
    /// e.g.:
    ///
    /// $ hc dna unpack ./some/dir/my-dna.dna
    ///
    /// creates a new directory `./some/dir/my-dna`, containining a new `dna.yaml`
    /// manifest
    // #[structopt(short = "u", long)]
    Unpack {
        /// The path to the bundle to unpack
        path: std::path::PathBuf,

        /// Specify the directory for the unpacked content
        ///
        /// If not specified, the directory will be placed alongside the
        /// bundle file, with the same name as the bundle file name.
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,

        /// Don't attempt to parse the manifest. Useful if you have a manifest
        /// of an outdated format. This command will allow you to unpack the
        /// manifest so that it may be modified and repacked into a valid bundle.
        #[structopt(short = "r", long)]
        raw: bool,

        /// Overwrite an existing directory, if one exists
        #[structopt(short = "f", long)]
        force: bool,
    },
}

/// Work with Holochain hApp bundles
#[derive(Debug, StructOpt)]
pub enum HcAppBundle {
    /// Create a new, empty Holochain app (hApp) working directory and create a new
    /// sample `happ.yaml` manifest inside.
    Init {
        /// The path to create the working directory
        path: PathBuf,
    },

    /// Pack into the `[name].happ` bundle according to the `happ.yaml` manifest,
    /// found inside the working directory. The `[name]` is taken from the `name`
    /// property of the manifest file.
    ///
    /// e.g.:
    ///
    /// $ hc app pack ./some/directory/foo
    ///
    /// creates a file `./some/directory/foo/[name].happ`, based on
    /// `./some/directory/foo/happ.yaml`
    Pack {
        /// The path to the working directory containing a `happ.yaml` manifest
        path: std::path::PathBuf,

        /// Specify the output path for the packed bundle file
        ///
        /// If not specified, the `[name].happ` bundle will be placed inside the
        /// provided working directory.
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,

        /// Also run `dna pack` on all DNAs manifests
        /// that have their location bundled
        #[structopt(short, long)]
        recursive: bool,
    },

    /// Unpack parts of the `.happ` bundle file into a specific directory.
    ///
    /// e.g.:
    ///
    /// $ hc app unpack ./some/dir/my-app.happ
    ///
    /// creates a new directory `./some/dir/my-app`, containining a new `happ.yaml`
    /// manifest
    // #[structopt(short = "u", long)]
    Unpack {
        /// The path to the bundle to unpack
        path: std::path::PathBuf,

        /// Specify the directory for the unpacked content
        ///
        /// If not specified, the directory will be placed alongside the
        /// bundle file, with the same name as the bundle file name.
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,

        /// Don't attempt to parse the manifest. Useful if you have a manifest
        /// of an outdated format. This command will allow you to unpack the
        /// manifest so that it may be modified and repacked into a valid bundle.
        #[structopt(short = "r", long)]
        raw: bool,

        /// Overwrite an existing directory, if one exists
        #[structopt(short = "f", long)]
        force: bool,
    },
}

/// Work with Holochain Web-hApp bundles
#[derive(Debug, StructOpt)]
pub enum HcWebAppBundle {
    /// Create a new, empty Holochain web app working directory and create a new
    /// sample `web-happ.yaml` manifest inside.
    Init {
        /// The path to create the working directory
        path: PathBuf,
    },

    /// Pack into the `[name].webhapp` bundle according to the `web-happ.yaml` manifest,
    /// found inside the working directory. The `[name]` is taken from the `name`
    /// property of the manifest file.
    ///
    /// e.g.:
    ///
    /// $ hc web-app pack ./some/directory/foo
    ///
    /// creates a file `./some/directory/foo/[name].webhapp`, based on
    /// `./some/directory/foo/web-happ.yaml`
    Pack {
        /// The path to the working directory containing a `web-happ.yaml` manifest
        path: std::path::PathBuf,

        /// Specify the output path for the packed bundle file
        ///
        /// If not specified, the `[name].webhapp` bundle will be placed inside the
        /// provided working directory.
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,

        /// Also run `app pack` and `dna pack` on all App and DNAs manifests
        /// that have their location bundled
        #[structopt(short, long)]
        recursive: bool,
    },

    /// Unpack parts of the `.webhapp` bundle file into a specific directory.
    ///
    /// e.g.:
    ///
    /// $ hc web-app unpack ./some/dir/my-app.webhapp
    ///
    /// creates a new directory `./some/dir/my-app`, containining a new `web-happ.yaml`
    /// manifest
    // #[structopt(short = "u", long)]
    Unpack {
        /// The path to the bundle to unpack
        path: std::path::PathBuf,

        /// Specify the directory for the unpacked content
        ///
        /// If not specified, the directory will be placed alongside the
        /// bundle file, with the same name as the bundle file name.
        #[structopt(short = "o", long)]
        output: Option<PathBuf>,

        /// Don't attempt to parse the manifest. Useful if you have a manifest
        /// of an outdated format. This command will allow you to unpack the
        /// manifest so that it may be modified and repacked into a valid bundle.
        #[structopt(short = "r", long)]
        raw: bool,

        /// Overwrite an existing directory, if one exists
        #[structopt(short = "f", long)]
        force: bool,
    },
}

impl HcDnaBundle {
    /// Run this command
    pub async fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Init { path } => {
                crate::init::init_dna(path).await?;
            }
            Self::Pack { path, output } => {
                let name = get_dna_name(&path).await?;
                let (bundle_path, _) =
                    crate::packing::pack::<ValidatedDnaManifest>(&path, output, name).await?;
                println!("Wrote bundle {}", bundle_path.to_string_lossy());
            }
            Self::Unpack {
                path,
                output,
                raw,
                force,
            } => {
                let dir_path = if raw {
                    crate::packing::unpack_raw(
                        DNA_BUNDLE_EXT,
                        &path,
                        output,
                        ValidatedDnaManifest::path().as_ref(),
                        force,
                    )
                    .await?
                } else {
                    crate::packing::unpack::<ValidatedDnaManifest>(
                        DNA_BUNDLE_EXT,
                        &path,
                        output,
                        force,
                    )
                    .await?
                };
                println!("Unpacked to directory {}", dir_path.to_string_lossy());
            }
        }
        Ok(())
    }
}

impl HcAppBundle {
    /// Run this command
    pub async fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Init { path } => {
                crate::init::init_app(path).await?;
            }
            Self::Pack {
                path,
                output,
                recursive,
            } => {
                let name = get_app_name(&path).await?;

                if recursive {
                    app_pack_recursive(&path).await?;
                }

                let (bundle_path, _) =
                    crate::packing::pack::<AppManifest>(&path, output, name).await?;
                println!("Wrote bundle {}", bundle_path.to_string_lossy());
            }
            Self::Unpack {
                path,
                output,
                raw,
                force,
            } => {
                let dir_path = if raw {
                    crate::packing::unpack_raw(
                        APP_BUNDLE_EXT,
                        &path,
                        output,
                        AppManifest::path().as_ref(),
                        force,
                    )
                    .await?
                } else {
                    crate::packing::unpack::<AppManifest>(APP_BUNDLE_EXT, &path, output, force)
                        .await?
                };
                println!("Unpacked to directory {}", dir_path.to_string_lossy());
            }
        }
        Ok(())
    }
}

impl HcWebAppBundle {
    /// Run this command
    pub async fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Init { path } => {
                crate::init::init_web_app(path).await?;
            }
            Self::Pack {
                path,
                output,
                recursive,
            } => {
                let name = get_web_app_name(&path).await?;

                if recursive {
                    web_app_pack_recursive(&path).await?;
                }

                let (bundle_path, _) =
                    crate::packing::pack::<WebAppManifest>(&path, output, name).await?;
                println!("Wrote bundle {}", bundle_path.to_string_lossy());
            }
            Self::Unpack {
                path,
                output,
                raw,
                force,
            } => {
                let dir_path = if raw {
                    crate::packing::unpack_raw(
                        WEB_APP_BUNDLE_EXT,
                        &path,
                        output,
                        WebAppManifest::path().as_ref(),
                        force,
                    )
                    .await?
                } else {
                    crate::packing::unpack::<WebAppManifest>(
                        WEB_APP_BUNDLE_EXT,
                        &path,
                        output,
                        force,
                    )
                    .await?
                };
                println!("Unpacked to directory {}", dir_path.to_string_lossy());
            }
        }
        Ok(())
    }
}

async fn get_dna_name(manifest_path: &Path) -> HcBundleResult<String> {
    let manifest_path = manifest_path.to_path_buf();
    let manifest_path = manifest_path.join(&ValidatedDnaManifest::path());
    let manifest_yaml = ffs::read_to_string(&manifest_path).await?;
    let manifest: DnaManifest = serde_yaml::from_str(&manifest_yaml)?;
    Ok(manifest.name())
}

async fn get_app_name(manifest_path: &Path) -> HcBundleResult<String> {
    let manifest_path = manifest_path.to_path_buf();
    let manifest_path = manifest_path.join(&AppManifest::path());
    let manifest_yaml = ffs::read_to_string(&manifest_path).await?;
    let manifest: AppManifest = serde_yaml::from_str(&manifest_yaml)?;
    Ok(manifest.app_name().to_string())
}

async fn get_web_app_name(manifest_path: &Path) -> HcBundleResult<String> {
    let manifest_path = manifest_path.to_path_buf();
    let manifest_path = manifest_path.join(&WebAppManifest::path());
    let manifest_yaml = ffs::read_to_string(&manifest_path).await?;
    let manifest: WebAppManifest = serde_yaml::from_str(&manifest_yaml)?;
    Ok(manifest.app_name().to_string())
}

// Pack the app's manifest and all its DNAs if their location is bundled
async fn web_app_pack_recursive(web_app_workdir_path: &PathBuf) -> anyhow::Result<()> {
    let canonical_web_app_workdir_path = ffs::canonicalize(web_app_workdir_path).await?;

    let web_app_manifest_path = canonical_web_app_workdir_path.join(WebAppManifest::path());

    let web_app_manifest: WebAppManifest =
        serde_yaml::from_reader(std::fs::File::open(&web_app_manifest_path)?)?;

    let app_bundle_location = web_app_manifest.happ_bundle_location();

    if let Location::Bundled(mut bundled_app_location) = app_bundle_location {
        // Remove the "APP_NAME.happ" portion of the path
        bundled_app_location.pop();

        // Join the web-app manifest location with the location of the app's workdir location
        let app_workdir_location = PathBuf::new()
            .join(web_app_workdir_path)
            .join(bundled_app_location);

        // Pack all the bundled DNAs and the app's manifest
        HcAppBundle::Pack {
            path: ffs::canonicalize(app_workdir_location).await?,
            output: None,
            recursive: true,
        }
        .run()
        .await?;
    }

    Ok(())
}

// Pack all the app's DNAs if their location is bundled
async fn app_pack_recursive(app_workdir_path: &PathBuf) -> anyhow::Result<()> {
    let app_workdir_path = ffs::canonicalize(app_workdir_path).await?;

    let app_manifest_path = app_workdir_path.join(AppManifest::path());
    let f = std::fs::File::open(&app_manifest_path)?;

    let manifest: AppManifest = serde_yaml::from_reader(f)?;

    let dnas_workdir_locations =
        bundled_dnas_workdir_locations(&app_manifest_path, &manifest).await?;

    for dna_workdir_location in dnas_workdir_locations {
        HcDnaBundle::Pack {
            path: dna_workdir_location,
            output: None,
        }
        .run()
        .await?;
    }

    Ok(())
}

// Returns all the locations of the workdirs for the bundled DNAs in the given app manifest
async fn bundled_dnas_workdir_locations(
    app_manifest_path: &PathBuf,
    app_manifest: &AppManifest,
) -> anyhow::Result<Vec<PathBuf>> {
    let mut dna_locations: Vec<PathBuf> = vec![];

    let mut app_workdir_location = app_manifest_path.clone();
    app_workdir_location.pop();

    for app_role in app_manifest.app_roles() {
        if let Some(Location::Bundled(mut dna_bundle_location)) = app_role.dna.location {
            // Remove the "DNA_NAME.yaml" portion of the path
            dna_bundle_location.pop();

            // Join the app's workdir location with the DNA bundle location, which is relative to it
            let dna_workdir_location = PathBuf::new()
                .join(&app_workdir_location)
                .join(&dna_bundle_location);

            dna_locations.push(ffs::canonicalize(dna_workdir_location).await?);
        }
    }

    Ok(dna_locations)
}
