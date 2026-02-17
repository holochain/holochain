use influxive_downloader::*;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod tgt {
    use super::*;
    pub const DL_DB: Option<DownloadSpec> = Some(DownloadSpec {
        url: "https://dl.influxdata.com/influxdb/releases/influxdb2-2.7.6_linux_amd64.tar.gz",
        archive: Archive::TarGz {
            inner_path: "influxdb2-2.7.6/usr/bin/influxd",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "a29d56dbd18edeeb893f61fa0517f5d9140d2e073f2ecf805912f4b91f308825"
        )),
        file_hash: Hash::Sha2_256(&hex_literal::hex!(
            "5c8fcb17accb9b0e689e0c6259aaace16ac42e5c0a2c99cef158de843d14d759"
        )),
        file_prefix: "influxd",
        file_extension: "",
    });
    pub const DL_CLI: Option<DownloadSpec> = Some(DownloadSpec {
        url:
            "https://dl.influxdata.com/influxdb/releases/influxdb2-client-2.7.3-linux-amd64.tar.gz",
        archive: Archive::TarGz {
            inner_path: "influx",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "a266f304547463b6bc7886bf45e37d252bcc0ceb3156ab8d78c52561558fbfe6"
        )),
        file_hash: Hash::Sha2_256(&hex_literal::hex!(
            "63a2aa0112bba8cd357656b5393c5e6655da6c85590374342b5f0ef14c60fa75"
        )),
        file_prefix: "influx",
        file_extension: "",
    });
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
mod tgt {
    use super::*;
    pub const DL_DB: Option<DownloadSpec> = Some(DownloadSpec {
        url: "https://dl.influxdata.com/influxdb/releases/influxdb2-2.7.6_linux_arm64.tar.gz",
        archive: Archive::TarGz {
            inner_path: "influxdb2-2.7.6/usr/bin/influxd",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "96b5574a2772da7d54f496d881e741f3339027a563502e55d8084504b0e22e90"
        )),
        file_hash: Hash::Sha2_256(&hex_literal::hex!(
            "e2d8b81c11e6bec68bc61adb224ba58d87be951c6a0ce9f9907c55a3b69ed864"
        )),
        file_prefix: "influxd",
        file_extension: "",
    });
    pub const DL_CLI: Option<DownloadSpec> = Some(DownloadSpec {
        url:
            "https://dl.influxdata.com/influxdb/releases/influxdb2-client-2.7.3-linux-arm64.tar.gz",
        archive: Archive::TarGz {
            inner_path: "influx",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "d5d09f5279aa32d692362cd096d002d787b3983868487e6f27379b1e205b4ba2"
        )),
        file_hash: Hash::Sha2_256(&hex_literal::hex!(
            "5dce0d53d84ac5c2ac93acc87585a9da44ba02e5f618418ce8a79d643c372234"
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
        url: "https://dl.influxdata.com/influxdb/releases/influxdb2-2.7.6_darwin_amd64.tar.gz",
        archive: Archive::TarGz {
            inner_path: "influxdb2-2.7.6/influxd",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "f484a41ad390ccae7a4cc99960ed0eb6a0309487696cf646b63122c96dbd139d"
        )),
        file_hash: Hash::Sha2_256(&hex_literal::hex!(
            "01d6a032a9508c8e860726824e9c50e177d3afa5333c69c753d6c776c5d89e4a"
        )),
        file_prefix: "influxd",
        file_extension: "",
    });
    pub const DL_CLI: Option<DownloadSpec> = Some(DownloadSpec {
        url:
            "https://dl.influxdata.com/influxdb/releases/influxdb2-client-2.7.3-darwin-amd64.tar.gz",
        archive: Archive::TarGz {
            inner_path: "influx",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "4d8297fc9e4ba15e432189295743c399a3e2647e9621bf36c68fbae8873f51b1"
        )),
        file_hash: Hash::Sha2_256(&hex_literal::hex!(
            "d1744c495f4cba666f79275419d9d48178fb7892113884fbd1cca3d6fc9b4009"
        )),
        file_prefix: "influx",
        file_extension: "",
    });
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod tgt {
    use super::*;
    pub const DL_DB: Option<DownloadSpec> = Some(DownloadSpec {
        url: "https://dl.influxdata.com/influxdb/releases/influxdb2-2.7.6-windows.zip",
        archive: Archive::Zip {
            inner_path: "influxd.exe",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "a874451d9e41dbcd63486c382b9d8bd4c3e06d7ebfdd78b0c7cdfa16bf7e5df3"
        )),
        file_hash: Hash::Sha2_256(&hex_literal::hex!(
            "c6ad343c46eff73fcebcbeead06d30564293c6ece2dfaa829dfe1811370450ed"
        )),
        file_prefix: "influxd",
        file_extension: ".exe",
    });
    pub const DL_CLI: Option<DownloadSpec> = Some(DownloadSpec {
        url: "https://dl.influxdata.com/influxdb/releases/influxdb2-client-2.7.3-windows-amd64.zip",
        archive: Archive::Zip {
            inner_path: "influx.exe",
        },
        archive_hash: Hash::Sha2_256(&hex_literal::hex!(
            "a9265771a2693269e50eeaf2ac82ac01d44305c6c6a5b425cf63e8289b6e89c4"
        )),
        file_hash: Hash::Sha2_256(&hex_literal::hex!(
            "829bb2657149436a88a959ea223c9f85bb25431fcf2891056522d9ec061f093e"
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
