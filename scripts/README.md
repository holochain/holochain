# Test Metrics Uploader

Uploads JUnit XML test results to InfluxDB for time-series analysis of test performance and flakiness.

## Prerequisites

```bash
cd scripts
npm install
```

## Usage

### Dry Run (Preview Data)

Test the script without uploading to see what data would be sent:

```bash
./junit-to-influxdb.js ../junit.xml --dry-run --runner-name "local" --run-id "test-1"
```

### Upload to InfluxDB

```bash
./junit-to-influxdb.js ../junit.xml \
  --influx-url https://influx.example.com \
  --influx-org my-org \
  --influx-bucket test-results \
  --influx-token YOUR_TOKEN \
  --runner-name "GitHub Actions" \
  --run-id "$GITHUB_RUN_ID"
```

### Options

- `--influx-url <url>` - InfluxDB server URL
- `--influx-org <org>` - InfluxDB organization name
- `--influx-bucket <bucket>` - InfluxDB bucket name
- `--influx-token <token>` - InfluxDB authentication token
- `--runner-name <name>` - Test runner identifier (e.g., "GitHub Actions", "local")
- `--run-id <id>` - Unique run identifier (e.g., GitHub run ID, commit SHA)
- `--extra <json>` - Additional tags as JSON string (e.g., `'{"branch":"main"}'`)
- `--dry-run` - Parse and display data without uploading

## Data Model

### Measurement

`test_result`

### Tags (Indexed)

- `test_suite` - Test suite name
- `test_name` - Individual test name
- `class_name` - Test class/module name
- `status` - Test status: `passed`, `failed`, or `flaky`
- `runner_name` - Test runner identifier
- `run_id` - Run identifier
- Additional tags from `--extra` option

### Fields (Values)

- `duration` (float) - Test execution time in seconds
- `has_failure` (bool) - Whether test failed
- `has_flaky_failure` (bool) - Whether test was flaky (passed after retry)
- `suite_total_tests` (int) - Total tests in suite
- `suite_failures` (int) - Total failures in suite
- `suite_errors` (int) - Total errors in suite
- `suite_total_duration` (float) - Total suite duration
- `failure_message` (string) - Failure message if failed
- `failure_type` (string) - Failure type if failed
- `failure_details` (string) - Full failure details if failed
- `flaky_failure_*` - Same fields for flaky failures
- `system_out` (string) - Test stdout output
- `system_err` (string) - Test stderr output

### Timestamp

Extracted from JUnit XML test case timestamp.

## Generating Test Results

Run tests with nextest to generate JUnit XML:

```bash
cd /path/to/holochain
cargo nextest run
# Creates junit.xml in project root
```

## Example Workflow

```bash
# 1. Run tests
cargo nextest run -p holochain_cli_client

# 2. Preview data
cd scripts
./junit-to-influxdb.js ../junit.xml --dry-run --runner-name "local" --run-id "$(git rev-parse HEAD)"

# 3. Upload to InfluxDB
./junit-to-influxdb.js ../junit.xml \
  --influx-url "$INFLUX_URL" \
  --influx-org "$INFLUX_ORG" \
  --influx-bucket "$INFLUX_BUCKET" \
  --influx-token "$INFLUX_TOKEN" \
  --runner-name "local" \
  --run-id "$(git rev-parse HEAD)" \
  --extra '{"branch":"'$(git branch --show-current)'"}'
```
