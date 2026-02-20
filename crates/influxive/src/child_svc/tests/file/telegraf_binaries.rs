use crate::child_svc::downloader::{Archive, DownloadSpec, Hash};
use hex_literal::hex;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const TELEGRAF_SPEC: DownloadSpec = DownloadSpec {
    url: "https://dl.influxdata.com/telegraf/releases/telegraf-1.28.5_linux_amd64.tar.gz",
    archive: Archive::TarGz {
        inner_path: "telegraf-1.28.5/usr/bin/telegraf",
    },
    archive_hash: Hash::Sha2_256(&hex!(
        "ae2f925e8e999299d4f4e6db7c20395813457edfb4128652d685cecb501ef669"
    )),
    file_prefix: "telegraf",
    file_extension: "",
};

#[cfg(all(
    any(target_os = "macos", target_os = "ios", target_os = "tvos"),
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub const TELEGRAF_SPEC: DownloadSpec = DownloadSpec {
    url: "https://dl.influxdata.com/telegraf/releases/telegraf-1.28.5_darwin_amd64.tar.gz",
    archive: Archive::TarGz {
        inner_path: "telegraf-1.28.5/usr/bin/telegraf",
    },
    archive_hash: Hash::Sha2_256(&hex!(
        "0848074b210d4a40e4b22f6a8b3c48450428ad02f9f796c1e2d55dee8d441c5b"
    )),
    file_prefix: "telegraf",
    file_extension: "",
};

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
pub const TELEGRAF_SPEC: DownloadSpec = DownloadSpec {
    url: "https://dl.influxdata.com/telegraf/releases/telegraf-1.28.5_windows_amd64.zip",
    archive: Archive::Zip {
        inner_path: "telegraf-1.28.5/telegraf.exe",
    },
    archive_hash: Hash::Sha2_256(&hex!(
        "e025bdd57bad5174f2490da47983eff4aa9f0a884343c0629d6ef774dcf2a892"
    )),
    file_prefix: "telegraf",
    file_extension: ".exe",
};
