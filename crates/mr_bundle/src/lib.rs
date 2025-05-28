//! Library for collecting bundling resources based on a manifest.
//!
//! Bundles created with Mr. Bundle are designed to be portable so
//! that they can be sent to other systems and unpacked there.
//!
//! A [`Bundle`] contains a [`Manifest`] as well as any number of arbitrary
//! opaque resources in the form of [`ResourceBytes`]. The manifest describes
//! the resources that should be included in the bundle. A Bundle can be
//! serialized and written to a file.
//!
//! With the `fs` feature, the `FileSystemBundler` can be used to work with
//! bundles on the file system.
//!
//! # Example: In-memory bundle
//!
//! A basic use of this library would be to create a bundle in-memory.
//!
//! ```rust
//! use std::collections::HashMap;
//! use serde::{Deserialize, Serialize};
//! use mr_bundle::{Bundle, Manifest, ResourceIdentifier};
//!
//! // Define your manifest
//! #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
//! struct MyManifest {
//!     video: String,
//!     audio: String,
//! }
//!
//! // Implement the Manifest trait for MyManifest
//! impl Manifest for MyManifest {
//!     fn generate_resource_ids(&mut self) -> HashMap<ResourceIdentifier, String> {
//!         [self.video.clone(), self.audio.clone()].into_iter().map(|r| {
//!            (r.clone(), r.clone())
//!         }).collect()
//!     }
//!
//!     fn resource_ids(&self) -> Vec<ResourceIdentifier> {
//!         [self.video.clone(), self.audio.clone()].into_iter().collect()
//!     }
//!
//!     fn file_name() -> &'static str {
//!         "example.yaml"
//!     }
//!
//!     fn bundle_extension() -> &'static str {
//!         "bundle"
//!     }
//! }
//!
//! let bundle = Bundle::new(
//!   MyManifest {
//!       video: "audio_sample".into(),
//!       audio: "video_sample".into(),
//!   },
//!   vec![(
//!      "audio_sample".to_string(), vec![1, 2, 3].into()
//!   ), (
//!      "video_sample".to_string(), vec![44, 54, 23].into()
//!   )]
//! ).unwrap();
//!
//! // Serialize the bundle to a byte vector
//! let bytes = bundle.pack().unwrap();
//!
//! // Then do something with the bytes...
//! ```
//!
//! # Example: Bundle to the file system
//!
//!
//! ```rust,no_run
//! use std::collections::HashMap;
//! use serde::{Deserialize, Serialize};
//! use mr_bundle::{resource_id_for_path, Bundle, FileSystemBundler, Manifest, ResourceIdentifier};
//!
//! # #[tokio::main]
//! # async fn main() {
//! // Define your manifest
//! #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
//! struct MyManifest {
//!     video: String,
//!     audio: String,
//! }
//!
//! // Implement the Manifest trait for MyManifest
//! impl Manifest for MyManifest {
//!     fn generate_resource_ids(&mut self) -> HashMap<ResourceIdentifier, String> {
//!         let mut out = HashMap::new();
//!
//!         let audio_id = resource_id_for_path(&self.audio).unwrap_or("audio-id".to_string());
//!         out.insert(audio_id.clone(), self.audio.clone());
//!         self.audio = audio_id;
//!
//!         let video_id = resource_id_for_path(&self.video).unwrap_or("video-id".to_string());
//!         out.insert(video_id.clone(), self.video.clone());
//!         self.video = video_id;
//!
//!         out
//!     }
//!
//!     fn resource_ids(&self) -> Vec<ResourceIdentifier> {
//!         [
//!             resource_id_for_path(&self.audio).unwrap_or("audio-id".to_string()),
//!             resource_id_for_path(&self.video).unwrap_or("video-id".to_string())
//!         ].into_iter().collect()
//!     }
//!
//!     fn file_name() -> &'static str {
//!         "example.yaml"
//!     }
//!
//!     fn bundle_extension() -> &'static str {
//!         "bundle"
//!     }
//! }
//!
//! // Create an example manifest, and note that the resource paths would also need to exist.
//! std::fs::write("./example.yaml", r#"
//! audio: ./audio-sample.mp3
//! video: ./video-sample.mp4
//! "#).unwrap();
//!
//! // Then create a bundle using the manifest.
//! // The resulting bundle will be written to the file system.
//! FileSystemBundler::bundle_to::<MyManifest>(
//!     "./example.yaml",
//!     "./packaging/example.bundle",
//! ).await.unwrap();
//!
//! // The bundle will now exist on the file system.
//! assert!(std::fs::exists("./packaging/example.bundle").unwrap());
//! # }
//! ```
//!

#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod bundle;
pub mod error;
#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
mod fs;
mod manifest;
mod pack;

pub use bundle::{resource::ResourceBytes, Bundle, ResourceMap};
pub use manifest::{Manifest, ResourceIdentifier};
pub use pack::{pack, unpack};

#[cfg(feature = "fs")]
pub use fs::{resource_id_for_path, FileSystemBundler};
