//! CLI definitions.

use crate::error::HcBundleResult;
use anyhow::Context;
use clap::{Parser, Subcommand};
use holochain_types::dna::DnaBundle;
use holochain_types::prelude::{AppManifest, DnaManifest, ValidatedDnaManifest};
use holochain_types::web_app::WebAppManifest;
use holochain_util::ffs;
use mr_bundle::{FileSystemBundler, Manifest};
use std::path::Path;
use std::path::PathBuf;

/// The file extension to use for DNA bundles.
pub const DNA_BUNDLE_EXT: &str = "dna";

/// The file extension to use for hApp bundles.
pub const APP_BUNDLE_EXT: &str = "happ";

/// The file extension to use for Web-hApp bundles.
pub const WEB_APP_BUNDLE_EXT: &str = "webhapp";

/// Work with Holochain DNA bundles.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct HcDnaBundle {
    /// The `hc dna` subcommand to run.
    #[command(subcommand)]
    pub subcommand: HcDnaBundleSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum HcDnaBundleSubcommand {
    /// Create a new, empty Holochain DNA bundle working directory and create a new
    /// sample `dna.yaml` manifest inside.
    Init {
        /// The path to create the working directory.
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
    /// `./some/directory/foo/dna.yaml`.
    Pack {
        /// The path to the working directory containing a `dna.yaml` manifest.
        path: PathBuf,

        /// Specify the output path for the packed bundle file.
        ///
        /// If not specified, the `[name].dna` bundle will be placed inside the
        /// provided working directory.
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,
    },

    /// Unpack parts of the `.dna` bundle file into a specific directory.
    ///
    /// e.g.:
    ///
    /// $ hc dna unpack ./some/dir/my-dna.dna
    ///
    /// creates a new directory `./some/dir/my-dna`, containining a new `dna.yaml`
    /// manifest.
    // #[arg(short = 'u', long)]
    Unpack {
        /// The path to the bundle to unpack.
        path: std::path::PathBuf,

        /// Specify the directory for the unpacked content.
        ///
        /// If not specified, the directory will be placed alongside the
        /// bundle file, with the same name as the bundle file name.
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,

        /// Don't attempt to parse the manifest. Useful if you have a manifest
        /// of an outdated format. This command will allow you to unpack the
        /// manifest so that it may be modified and repacked into a valid bundle.
        #[arg(short = 'r', long)]
        raw: bool,

        /// Overwrite an existing directory, if one exists.
        #[arg(short = 'f', long)]
        force: bool,
    },

    /// Print the schema for a DNA manifest
    Schema,
    /// Print the Base64 hash for a DNA file
    Hash {
        /// The path to the dna file.
        path: std::path::PathBuf,
    },
}

/// Work with Holochain hApp bundles.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct HcAppBundle {
    /// The `hc app` subcommand to run.
    #[command(subcommand)]
    pub subcommand: HcAppBundleSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum HcAppBundleSubcommand {
    /// Create a new, empty Holochain app (hApp) working directory and create a new
    /// sample `happ.yaml` manifest inside.
    Init {
        /// The path to create the working directory.
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
    /// `./some/directory/foo/happ.yaml`.
    Pack {
        /// The path to the working directory containing a `happ.yaml` manifest.
        path: PathBuf,

        /// Specify the output path for the packed bundle file.
        ///
        /// If not specified, the `[name].happ` bundle will be placed inside the
        /// provided working directory.
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,

        /// Also run `dna pack` on all DNA manifests
        /// to be bundled into this hApp.
        /// There must exist a `dna.yaml` file in the same directory
        /// as each of the DNA files specified in the manifest.
        #[arg(short, long)]
        recursive: bool,
    },

    /// Unpack parts of the `.happ` bundle file into a specific directory.
    ///
    /// e.g.:
    ///
    /// $ hc app unpack ./some/dir/my-app.happ
    ///
    /// creates a new directory `./some/dir/my-app`, containining a new `happ.yaml`
    /// manifest.
    // #[arg(short = 'u', long)]
    Unpack {
        /// The path to the bundle to unpack.
        path: PathBuf,

        /// Specify the directory for the unpacked content.
        ///
        /// If not specified, the directory will be placed alongside the
        /// bundle file, with the same name as the bundle file name.
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,

        /// Don't attempt to parse the manifest. Useful if you have a manifest
        /// of an outdated format. This command will allow you to unpack the
        /// manifest so that it may be modified and repacked into a valid bundle.
        #[arg(short = 'r', long)]
        raw: bool,

        /// Overwrite an existing directory, if one exists.
        #[arg(short = 'f', long)]
        force: bool,
    },

    /// Print the schema for a hApp manifest
    Schema,
}

/// Work with Holochain web-hApp bundles.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct HcWebAppBundle {
    /// The `hc web-app` subcommand to run.
    #[command(subcommand)]
    pub subcommand: HcWebAppBundleSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum HcWebAppBundleSubcommand {
    /// Create a new, empty Holochain web app working directory and create a new
    /// sample `web-happ.yaml` manifest inside.
    Init {
        /// The path to create the working directory.
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
    /// `./some/directory/foo/web-happ.yaml`.
    Pack {
        /// The path to the working directory containing a `web-happ.yaml` manifest.
        path: std::path::PathBuf,

        /// Specify the output path for the packed bundle file.
        ///
        /// If not specified, the `[name].webhapp` bundle will be placed inside the
        /// provided working directory.
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,

        /// Also run `app pack` and `dna pack` on all app and DNA manifests
        /// to be bundled into this hApp.
        /// There must exist a `happ.yaml` file file in the same directory
        /// as the hApp file specified in the manifest,
        /// as well as `dna.yaml` files in the same directories
        /// as each of the DNA files specified in the hApps' manifests.
        #[arg(short, long)]
        recursive: bool,
    },

    /// Unpack parts of the `.webhapp` bundle file into a specific directory.
    ///
    /// e.g.:
    ///
    /// $ hc web-app unpack ./some/dir/my-app.webhapp
    ///
    /// creates a new directory `./some/dir/my-app`, containining a new `web-happ.yaml`
    /// manifest.
    // #[arg(short = 'u', long)]
    Unpack {
        /// The path to the bundle to unpack.
        path: std::path::PathBuf,

        /// Specify the directory for the unpacked content.
        ///
        /// If not specified, the directory will be placed alongside the
        /// bundle file, with the same name as the bundle file name.
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,

        /// Don't attempt to parse the manifest. Useful if you have a manifest
        /// of an outdated format. This command will allow you to unpack the
        /// manifest so that it may be modified and repacked into a valid bundle.
        #[arg(short = 'r', long)]
        raw: bool,

        /// Overwrite an existing directory, if one exists.
        #[arg(short = 'f', long)]
        force: bool,
    },

    /// Print the schema for a web hApp manifest
    Schema,
}

// These impls are here to make the code for the three `Hc_Bundle` subcommand wrappers
// somewhat consistent with the main subcommand wrapper and that of `hc-sandbox`,
// in which it's the wrapper struct that contains the `run` function.
// The reason the `run` function is on these subcommands' sub-subcommand enums
// is that the recursive packing functions call them directly on the variants
// and don't want to bother instantiating a wrapper just for that.

impl HcDnaBundle {
    /// Run this subcommand, passing off all the work to the sub-sub-command enum
    pub async fn run(self) -> anyhow::Result<()> {
        self.subcommand.run().await
    }
}

impl HcAppBundle {
    /// Run this subcommand, passing off all the work to the sub-sub-command enum
    pub async fn run(self) -> anyhow::Result<()> {
        self.subcommand.run().await
    }
}

impl HcWebAppBundle {
    /// Run this subcommand, passing off all the work to the sub-sub-command enum
    pub async fn run(self) -> anyhow::Result<()> {
        self.subcommand.run().await
    }
}

impl HcDnaBundleSubcommand {
    /// Run this command
    pub async fn run(self) -> anyhow::Result<()> {
        match self {
            Self::Init { path } => {
                crate::init::init_dna(path).await?;
            }
            Self::Pack { path, output } => {
                let name = get_dna_name(&path).await?;
                let bundle_path =
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
                    crate::packing::expand_unknown_bundle(
                        &path,
                        DNA_BUNDLE_EXT,
                        ValidatedDnaManifest::file_name(),
                        output,
                        force,
                    )
                    .await?
                } else {
                    crate::packing::expand_bundle::<ValidatedDnaManifest>(&path, output, force)
                        .await?
                };
                println!("Unpacked to directory {}", dir_path.to_string_lossy());
            }
            Self::Schema => {
                let schema = schemars::schema_for!(DnaManifest);
                let schema_string = serde_json::to_string_pretty(&schema)
                    .context("Failed to pretty print schema")?;

                println!("{}", schema_string);
            }
            Self::Hash { path } => {
                let bundle = FileSystemBundler::load_from::<ValidatedDnaManifest>(path)
                    .await
                    .map(DnaBundle::from)?;
                let dna_hash_b64 = bundle.to_dna_file().await?.0.dna_hash().to_string();
                println!("{}", dna_hash_b64);
            }
        }
        Ok(())
    }
}

impl HcAppBundleSubcommand {
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

                let bundle_path = crate::packing::pack::<AppManifest>(&path, output, name).await?;
                println!("Wrote bundle {}", bundle_path.to_string_lossy());
            }
            Self::Unpack {
                path,
                output,
                raw,
                force,
            } => {
                let dir_path = if raw {
                    crate::packing::expand_unknown_bundle(
                        &path,
                        APP_BUNDLE_EXT,
                        AppManifest::file_name(),
                        output,
                        force,
                    )
                    .await?
                } else {
                    crate::packing::expand_bundle::<AppManifest>(&path, output, force).await?
                };
                println!("Unpacked to directory {}", dir_path.to_string_lossy());
            }
            Self::Schema => {
                let schema = schemars::schema_for!(AppManifest);
                let schema_string = serde_json::to_string_pretty(&schema)
                    .context("Failed to pretty print schema")?;

                println!("{}", schema_string);
            }
        }
        Ok(())
    }
}

impl HcWebAppBundleSubcommand {
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

                let bundle_path =
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
                    crate::packing::expand_unknown_bundle(
                        &path,
                        WEB_APP_BUNDLE_EXT,
                        WebAppManifest::file_name(),
                        output,
                        force,
                    )
                    .await?
                } else {
                    crate::packing::expand_bundle::<WebAppManifest>(&path, output, force).await?
                };
                println!("Unpacked to directory {}", dir_path.to_string_lossy());
            }
            Self::Schema => {
                let schema = schemars::schema_for!(WebAppManifest);
                let schema_string = serde_json::to_string_pretty(&schema)
                    .context("Failed to pretty print schema")?;

                println!("{}", schema_string);
            }
        }
        Ok(())
    }
}

/// Load a [ValidatedDnaManifest] manifest from the given path and return its `name` field.
pub async fn get_dna_name(manifest_path: &Path) -> HcBundleResult<String> {
    let manifest_path = manifest_path.to_path_buf();
    let manifest_path = manifest_path.join(ValidatedDnaManifest::file_name());
    let manifest_yaml = ffs::read_to_string(&manifest_path).await?;
    let manifest: DnaManifest = serde_yaml::from_str(&manifest_yaml)?;
    Ok(manifest.name())
}

/// Load an [AppManifest] manifest from the given path and return its `app_name` field.
pub async fn get_app_name(manifest_path: &Path) -> HcBundleResult<String> {
    let manifest_path = manifest_path.to_path_buf();
    let manifest_path = manifest_path.join(AppManifest::file_name());
    let manifest_yaml = ffs::read_to_string(&manifest_path).await?;
    let manifest: AppManifest = serde_yaml::from_str(&manifest_yaml)?;
    Ok(manifest.app_name().to_string())
}

/// Load a [WebAppManifest] manifest from the given path and return its `app_name` field.
pub async fn get_web_app_name(manifest_path: &Path) -> HcBundleResult<String> {
    let manifest_path = manifest_path.to_path_buf();
    let manifest_path = manifest_path.join(WebAppManifest::file_name());
    let manifest_yaml = ffs::read_to_string(&manifest_path).await?;
    let manifest: WebAppManifest = serde_yaml::from_str(&manifest_yaml)?;
    Ok(manifest.app_name().to_string())
}

/// Pack the app's manifest and all its DNAs if their location is bundled
pub async fn web_app_pack_recursive(web_app_workdir_path: &PathBuf) -> anyhow::Result<()> {
    let canonical_web_app_workdir_path = ffs::canonicalize(web_app_workdir_path).await?;

    let web_app_manifest_path = canonical_web_app_workdir_path.join(WebAppManifest::file_name());

    let web_app_manifest: WebAppManifest =
        serde_yaml::from_reader(std::fs::File::open(&web_app_manifest_path)?)?;

    let app_bundle_location = web_app_manifest.happ_bundle_location();

    // Remove the "APP_NAME.happ" portion of the path
    let mut bundled_app_location = PathBuf::from(app_bundle_location);
    bundled_app_location.pop();

    // Join the web-app manifest location with the location of the app's workdir location
    let app_workdir_location = PathBuf::new()
        .join(web_app_workdir_path)
        .join(bundled_app_location);

    // Pack all the bundled DNAs and the app's manifest
    HcAppBundleSubcommand::Pack {
        path: ffs::canonicalize(app_workdir_location).await?,
        output: None,
        recursive: true,
    }
    .run()
    .await?;

    Ok(())
}

/// Pack all the app's DNAs if their location is bundled
pub async fn app_pack_recursive(app_workdir_path: &PathBuf) -> anyhow::Result<()> {
    let app_workdir_path = ffs::canonicalize(app_workdir_path).await?;

    let app_manifest_path = app_workdir_path.join(AppManifest::file_name());
    let f = std::fs::File::open(&app_manifest_path)?;

    let manifest: AppManifest = serde_yaml::from_reader(f)?;

    let dnas_workdir_locations =
        bundled_dnas_workdir_locations(&app_manifest_path, &manifest).await?;

    for dna_workdir_location in dnas_workdir_locations {
        HcDnaBundleSubcommand::Pack {
            path: dna_workdir_location,
            output: None,
        }
        .run()
        .await?;
    }

    Ok(())
}

/// Returns all the locations of the workdirs for the bundled DNAs in the given app manifest
pub async fn bundled_dnas_workdir_locations(
    app_manifest_path: &Path,
    app_manifest: &AppManifest,
) -> anyhow::Result<Vec<PathBuf>> {
    let mut dna_locations: Vec<PathBuf> = vec![];

    let mut app_workdir_location = app_manifest_path.to_path_buf();
    app_workdir_location.pop();

    for app_role in app_manifest.app_roles() {
        if let Some(file) = app_role.dna.path {
            let mut dna_bundle_location = PathBuf::from(file);

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
