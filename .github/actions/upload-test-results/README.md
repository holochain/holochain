# Upload Test Results to InfluxDB

This GitHub Action parses JUnit XML test results and uploads them to InfluxDB for time-series analysis and flaky test detection.

## Usage

```yaml
- name: Upload test results to InfluxDB
  uses: ./.github/actions/upload-test-results
  with:
    junit-file: junit.xml
    influx-url: ${{ secrets.INFLUX_URL }}
    influx-org: ${{ secrets.INFLUX_ORG }}
    influx-bucket: test-results
    influx-token: ${{ secrets.INFLUX_TOKEN }}
    runner-name: ${{ runner.os }}-${{ runner.arch }}
    tags: >-
      {
        "run_id": "${{ github.run_id }}",
        "test_target": "wasmer_sys",
        "branch": "${{ github.ref_name }}",
        "commit": "${{ github.sha }}"
      }
```

## Inputs

| Input | Description | Required | Default |
|-------|-------------|----------|----------|
| `junit-file` | Path to JUnit XML file | Yes | - |
| `influx-url` | InfluxDB URL | Yes | - |
| `influx-org` | InfluxDB organization | Yes | - |
| `influx-bucket` | InfluxDB bucket name | Yes | - |
| `influx-token` | InfluxDB authentication token | Yes | - |
| `runner-name` | Name of the CI runner | Yes | - |
| `tags` | Additional tags as JSON (e.g., run_id, test_target, branch) | No | `'{}'` |

## Data Model

Test results are stored in InfluxDB with the following structure:

**Measurement**: `test_result`

**Tags**:
- `test_suite`: Name of the test suite
- `test_name`: Name of the test
- `class_name`: Test class name
- `status`: Test status (passed, failed, flaky)
- `runner_name`: CI runner identifier
- Additional custom tags from the `tags` input (e.g., `run_id`, `test_target`, `branch`, `commit`)

**Fields**:
- `duration`: Test execution time (float)
- `has_failure`: 1 if test failed, 0 otherwise (int)
- `has_flaky_failure`: 1 if test was flaky, 0 otherwise (int)
- `failure_message`: Failure message (string, truncated to 32KB)
- `failure_type`: Failure type (string)
- `failure_details`: Detailed failure information (string, truncated to 32KB)
- `flaky_*`: Similar fields for flaky failures
- `system_out`: Standard output (string, truncated to 32KB)
- `system_err`: Standard error (string, truncated to 32KB)
- `suite_total_tests`: Total tests in suite (int)
- `suite_failures`: Total failures in suite (int)
- `suite_errors`: Total errors in suite (int)
- `suite_total_duration`: Total suite execution time (float)

## Development

This action uses Rollup to compile all dependencies into a single `dist/index.js` file.

To build:

```bash
cd .github/actions/upload-test-results
npm install
npm run build
```

The compiled `dist/index.js` file should be committed to the repository.
