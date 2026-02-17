use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct TelegrafLineProtocolConfig {
    influxdb_url: String,
    token: String,
    organization: String,
    bucket: String,
    metrics_file_path: PathBuf,
}

impl TelegrafLineProtocolConfig {
    pub fn new(
        influxdb_url: &str,
        token: &str,
        organization: &str,
        bucket: &str,
        metrics_file_path: &str,
    ) -> Self {
        Self {
            influxdb_url: influxdb_url.to_string(),
            token: token.to_string(),
            organization: organization.to_string(),
            bucket: bucket.to_string(),
            metrics_file_path: PathBuf::from(metrics_file_path),
        }
    }

    pub fn write_to_file(
        &self,
        config_output_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let config_content = self.build_content();

        // Create parent directories if they don't exist
        if let Some(parent) = config_output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = File::create(config_output_path)?;
        file.write_all(config_content.as_bytes())?;

        println!(
            "Telegraf Line Protocol configuration written to: {}",
            config_output_path.to_string_lossy(),
        );
        Ok(())
    }

    /// Builds Telegraf config file content based on config
    fn build_content(&self) -> String {
        format!(
            r#"# Generated Telegraf Configuration for Line Protocol Metrics

[global_tags]
  # Global tags can be specified here in key="value" format

[agent]
  interval = "5s"
  round_interval = true
  metric_batch_size = 1000
  metric_buffer_limit = 10000
  collection_jitter = "0s"
  flush_interval = "5s"
  flush_jitter = "0s"
  precision = ""
  hostname = ""
  omit_hostname = false
  quiet = false
#  logfile = "logs_telegraf.log"

# Configuration for InfluxDB v2 output
[[outputs.influxdb_v2]]
  ## The URLs of the InfluxDB cluster nodes.
  urls = ["{url}"]

  ## Token for authentication
  token = "{token}"

  ## Organization is the name of the organization you wish to write to
  organization = "{org}"

  ## Destination bucket to write into
  bucket = "{bucket}"

# Input plugin for reading Line Protocol metrics from file
[[inputs.file]]
  ## Files to parse each interval. Accept standard unix glob matching rules,
  ## as well as ** to match recursive files and directories.
  files = ["{filepath}"]

  ## Data format to consume.
  data_format = "influx"

  ## Character encoding to use when interpreting the file contents.  Invalid
  ## characters are replaced using the unicode replacement character.  When set
  ## to the empty string the encoding will be automatically determined.
  character_encoding = "utf-8"
"#,
            url = self.influxdb_url,
            token = self.token,
            org = self.organization,
            bucket = self.bucket,
            filepath = self
                .metrics_file_path
                .as_path()
                .to_string_lossy()
                .replace('\\', "\\\\"), // escape backslashes for Windows
        )
    }
}
