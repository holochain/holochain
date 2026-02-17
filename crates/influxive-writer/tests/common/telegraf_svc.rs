use crate::common::telegraf_binaries::TELEGRAF_SPEC;
use std::path::PathBuf;
use std::process::Stdio;

/// Spawns and handles a Telegraf child service
pub struct TelegrafSvc {
    process: std::process::Child,
}

impl TelegrafSvc {
    pub async fn spawn(
        config_path: &str,
        fallback_binary_dir: &str,
        once: bool,
    ) -> std::io::Result<Self> {
        // Ensure binary is available
        let filepath = TelegrafSvc::download_telegraf(fallback_binary_dir).await?;

        println!(
            "Starting Telegraf with config: {} | {}",
            config_path,
            filepath.to_string_lossy(),
        );

        let child = std::process::Command::new(&filepath)
            .arg("--config")
            .arg(config_path)
            .args(if once { vec!["--once"] } else { vec![] })
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;

        println!("Telegraf started successfully");

        Ok(Self { process: child })
    }

    async fn download_telegraf(binary_dir: &str) -> std::io::Result<PathBuf> {
        println!("Downloading from: {}", TELEGRAF_SPEC.url);
        let filepath = TELEGRAF_SPEC
            .download(PathBuf::from(binary_dir).as_path())
            .await?;
        println!(
            "Telegraf binary downloaded and extracted successfully to {}",
            filepath.display()
        );
        Ok(filepath)
    }
}

impl Drop for TelegrafSvc {
    fn drop(&mut self) {
        println!("Stopping Telegraf...");
        if let Err(err) = self.process.kill() {
            println!("Error killing Telegraf: {}", err);
        } else if let Err(err) = self.process.wait() {
            println!("Error waiting for Telegraf to exit: {}", err);
        } else {
            println!("Telegraf stopped");
        }
    }
}
