use crate::child_svc::downloader::{Archive, DownloadSpec, Hash};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod tgt {
    use super::*;
    pub const DL_DB: Option<DownloadSpec> = Some(DownloadSpec {
        url:
            "https://dl.influxdata.com/influxdb/releases/v2.8.0/influxdb2-2.8.0_linux_amd64.tar.gz",
        archive: Archive::TarGz {
            inner_path: "influxdb2-2.8.0/influxd",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "df28cb9d3cb47732908604d963b20271a3fb0e83f418976cc482f991e328957d"
        )),
        file_prefix: "influxd",
        file_extension: "",
    });
    pub const DL_CLI: Option<DownloadSpec> = Some(DownloadSpec {
        url:
            "https://dl.influxdata.com/influxdb/releases/influxdb2-client-2.7.5-linux-amd64.tar.gz",
        archive: Archive::TarGz {
            inner_path: "influx",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "496dffcd70bed2bb3dc3d614e3d9c97e312e092dfe0577d332027566bbb7d8cd"
        )),
        file_prefix: "influx",
        file_extension: "",
    });
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
mod tgt {
    use super::*;
    pub const DL_DB: Option<DownloadSpec> = Some(DownloadSpec {
        url:
            "https://dl.influxdata.com/influxdb/releases/v2.8.0/influxdb2-2.8.0_linux_arm64.tar.gz",
        archive: Archive::TarGz {
            inner_path: "influxd",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "263196a8970ceea2d8ff4b90c85555b7573a86c9d83f797a8dfc136e554edd70"
        )),
        file_prefix: "influxd",
        file_extension: "",
    });
    pub const DL_CLI: Option<DownloadSpec> = Some(DownloadSpec {
        url:
            "https://dl.influxdata.com/influxdb/releases/influxdb2-client-2.7.5-linux-arm64.tar.gz",
        archive: Archive::TarGz {
            inner_path: "influx",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "867c3cbabd63a34a9b1ac643fd5c5d268b694acc98e3b75fa5a78d63037097dd"
        )),
        file_prefix: "influx",
        file_extension: "",
    });
}

#[cfg(all(
    any(target_os = "macos", target_os = "ios", target_os = "tvos"),
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod tgt {
    use super::*;
    pub const DL_DB: Option<DownloadSpec> = Some(DownloadSpec {
        url:
            "https://dl.influxdata.com/influxdb/releases/v2.8.0/influxdb2-2.8.0_darwin_amd64.tar.gz",
        archive: Archive::TarGz {
            inner_path: "influxdb2-2.8.0/influxd",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "ab08199474f26c2feb636b993b1aaa3b159b9d849d0e66a69a812af193a042ec"
        )),
        file_prefix: "influxd",
        file_extension: "",
    });
    pub const DL_CLI: Option<DownloadSpec> = Some(DownloadSpec {
        url:
            "https://dl.influxdata.com/influxdb/releases/influxdb2-client-2.7.5-darwin-amd64.tar.gz",
        archive: Archive::TarGz {
            inner_path: "influx",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "873f4842a1c665d5a42b9b86bd80538599ae86aef5c4f17e6576cbd21608ca6c"
        )),
        file_prefix: "influx",
        file_extension: "",
    });
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod tgt {
    use super::*;
    pub const DL_DB: Option<DownloadSpec> = Some(DownloadSpec {
        url: "https://dl.influxdata.com/influxdb/releases/influxdb2-2.8.0-windows_amd64.zip",
        archive: Archive::Zip {
            inner_path: "influxd.exe",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "464d1240a7764c1c024021b5c5ac4a9943570929d615beec83a12f5e793becae"
        )),
        file_prefix: "influxd",
        file_extension: ".exe",
    });
    pub const DL_CLI: Option<DownloadSpec> = Some(DownloadSpec {
        url: "https://dl.influxdata.com/influxdb/releases/influxdb2-client-2.7.5-windows-amd64.zip",
        archive: Archive::Zip {
            inner_path: "influx.exe",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "7b965ea00514fc329a9b09d277445629f3bb5394e500ef4ae2c3dddd296de699"
        )),
        file_prefix: "influx",
        file_extension: ".exe",
    });
}

#[cfg(not(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(
        any(target_os = "macos", target_os = "ios", target_os = "tvos"),
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "windows", target_arch = "x86_64")
)))]
mod tgt {
    use super::*;
    pub const DL_DB: Option<DownloadSpec> = None;
    pub const DL_CLI: Option<DownloadSpec> = None;
}

pub use tgt::*;
