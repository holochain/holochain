#!/usr/bin/env node

const fs = require('fs');
const { parseStringPromise } = require('xml2js');
const { InfluxDB, Point } = require('@influxdata/influxdb-client');

async function convertJUnitToInfluxPoints(inputFile, testRun) {
  try {
    const xmlContent = fs.readFileSync(inputFile, 'utf8');
    const result = await parseStringPromise(xmlContent);

    const testsuites = result.testsuites;
    const testsuiteMetadata = {
      name: testsuites.$.name,
      totalTests: parseInt(testsuites.$.tests),
      failures: parseInt(testsuites.$.failures),
      errors: parseInt(testsuites.$.errors),
      uuid: testsuites.$.uuid,
      timestamp: testsuites.$.timestamp,
      totalDuration: parseFloat(testsuites.$.time)
    };

    const points = [];

    for (const testsuite of testsuites.testsuite) {
      const suiteName = testsuite.$.name;

      for (const testcase of testsuite.testcase) {
        const point = new Point('test_result')
          // Tags (indexed)
          .tag('test_suite', suiteName)
          .tag('test_name', testcase.$.name)
          .tag('class_name', testcase.$.classname)
          .tag('runner_name', testRun.runnerName || 'unknown')
          .tag('run_id', testRun.runId || 'unknown');

        // Add extra tags if provided
        if (testRun.extra) {
          try {
            const extraObj = JSON.parse(testRun.extra);
            for (const [key, value] of Object.entries(extraObj)) {
              point.tag(key, String(value));
            }
          } catch (e) {
            point.tag('extra', testRun.extra);
          }
        }

        // Fields (values)
        point.floatField('duration', parseFloat(testcase.$.time));
        point.intField('suite_total_tests', testsuiteMetadata.totalTests);
        point.intField('suite_failures', testsuiteMetadata.failures);
        point.intField('suite_errors', testsuiteMetadata.errors);
        point.floatField('suite_total_duration', testsuiteMetadata.totalDuration);

        let status = 'passed';
        let hasFailure = false;
        let hasFlakyFailure = false;

        // Check for flaky failure
        if (testcase.flakyFailure && testcase.flakyFailure.length > 0) {
          const flakyFailure = testcase.flakyFailure[0];
          hasFlakyFailure = true;
          status = 'flaky';

          point.floatField('flaky_failure_duration', parseFloat(flakyFailure.$.time));
          point.stringField('flaky_failure_message', flakyFailure.$.message || '');
          point.stringField('flaky_failure_type', flakyFailure.$.type || '');

          if (flakyFailure._ && flakyFailure._.length > 0) {
            // Truncate very long failure details to avoid InfluxDB limits
            const details = flakyFailure._.substring(0, 32000);
            point.stringField('flaky_failure_details', details);
          }

          if (flakyFailure['system-out'] && flakyFailure['system-out'].length > 0) {
            const out = flakyFailure['system-out'][0].substring(0, 32000);
            point.stringField('flaky_system_out', out);
          }
          if (flakyFailure['system-err'] && flakyFailure['system-err'].length > 0) {
            const err = flakyFailure['system-err'][0].substring(0, 32000);
            point.stringField('flaky_system_err', err);
          }
        }

        // Check for regular failure
        if (testcase.failure && testcase.failure.length > 0) {
          const failure = testcase.failure[0];
          hasFailure = true;
          status = 'failed';

          point.stringField('failure_message', failure.$.message || '');
          point.stringField('failure_type', failure.$.type || '');

          if (failure._ && failure._.length > 0) {
            const details = failure._.substring(0, 32000);
            point.stringField('failure_details', details);
          }
        }

        point.tag('status', status);
        point.booleanField('has_failure', hasFailure);
        point.booleanField('has_flaky_failure', hasFlakyFailure);

        // Add system output
        if (testcase['system-out'] && testcase['system-out'].length > 0) {
          const out = testcase['system-out'][0].substring(0, 32000);
          point.stringField('system_out', out);
        }

        if (testcase['system-err'] && testcase['system-err'].length > 0) {
          const err = testcase['system-err'][0].substring(0, 32000);
          point.stringField('system_err', err);
        }

        // Set timestamp from test case
        const timestamp = new Date(testcase.$.timestamp);
        point.timestamp(timestamp);

        points.push(point);
      }
    }

    return points;
  } catch (error) {
    console.error('Error converting file:', error);
    process.exit(1);
  }
}

async function uploadToInfluxDB(points, influxConfig, dryRun = false) {
  try {
    if (dryRun) {
      console.log(`\n📊 DRY RUN: Would upload ${points.length} points to InfluxDB`);
      console.log(`   URL: ${influxConfig.url}`);
      console.log(`   Org: ${influxConfig.org}`);
      console.log(`   Bucket: ${influxConfig.bucket}`);
      console.log('\n🔍 Sample points (first 3):\n');
      
      points.slice(0, 3).forEach((point, idx) => {
        console.log(`Point ${idx + 1}:`);
        console.log(point.toLineProtocol());
        console.log('');
      });
      
      // Show summary by status
      const statusCounts = {};
      points.forEach(point => {
        const line = point.toLineProtocol();
        const statusMatch = line.match(/status=([^,\s]+)/);
        if (statusMatch) {
          const status = statusMatch[1];
          statusCounts[status] = (statusCounts[status] || 0) + 1;
        }
      });
      
      console.log('\n📈 Summary by status:');
      for (const [status, count] of Object.entries(statusCounts)) {
        console.log(`   ${status}: ${count}`);
      }
      
      return;
    }

    const client = new InfluxDB({
      url: influxConfig.url,
      token: influxConfig.token
    });

    const writeApi = client.getWriteApi(influxConfig.org, influxConfig.bucket);
    writeApi.useDefaultTags({});

    console.log(`\n📤 Uploading ${points.length} points to InfluxDB...`);
    console.log(`   URL: ${influxConfig.url}`);
    console.log(`   Org: ${influxConfig.org}`);
    console.log(`   Bucket: ${influxConfig.bucket}`);

    // Write points
    writeApi.writePoints(points);

    // Flush and close
    await writeApi.close();

    console.log(`✅ Successfully uploaded ${points.length} test results!`);

  } catch (error) {
    console.error('\n❌ Error uploading to InfluxDB:', error);
    if (error.body) {
      console.error('   Details:', error.body);
    }
    process.exit(1);
  }
}

// Parse CLI arguments
function parseArgs(args) {
  const config = {
    inputFile: null,
    influxConfig: {
      url: null,
      org: null,
      bucket: null,
      token: null,
    },
    testRun: {
      runnerName: null,
      runId: null,
      extra: null,
    },
    dryRun: false
  };

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];

    if (arg === '--influx-url') {
      config.influxConfig.url = args[++i];
    } else if (arg === '--influx-org') {
      config.influxConfig.org = args[++i];
    } else if (arg === '--influx-bucket') {
      config.influxConfig.bucket = args[++i];
    } else if (arg === '--influx-token') {
      config.influxConfig.token = args[++i];
    } else if (arg === '--runner-name') {
      config.testRun.runnerName = args[++i];
    } else if (arg === '--run-id') {
      config.testRun.runId = args[++i];
    } else if (arg === '--extra') {
      config.testRun.extra = args[++i];
    } else if (arg === '--dry-run') {
      config.dryRun = true;
    } else if (!config.inputFile) {
      config.inputFile = arg;
    }
  }

  return config;
}

// CLI usage
const args = process.argv.slice(2);
if (args.length < 1) {
  console.log('Usage: junit-to-influxdb <input.xml> [options]');
  console.log('\nOptions:');
  console.log('  --influx-url <url>        InfluxDB URL');
  console.log('  --influx-org <org>        InfluxDB organization');
  console.log('  --influx-bucket <bucket>  InfluxDB bucket');
  console.log('  --influx-token <token>    InfluxDB auth token');
  console.log('  --runner-name <name>      Test runner name (e.g., "GitHub Actions")');
  console.log('  --run-id <id>             Test run ID (e.g., GitHub run ID)');
  console.log('  --extra <json>            Extra tags as JSON string');
  console.log('  --dry-run                 Parse and display data without uploading');
  console.log('\nExamples:');
  console.log('  # Dry run to preview data');
  console.log('  junit-to-influxdb junit.xml --dry-run --runner-name "local" --run-id "test-1"');
  console.log('');
  console.log('  # Upload to InfluxDB');
  console.log('  junit-to-influxdb junit.xml \\');
  console.log('    --influx-url https://influx.example.com \\');
  console.log('    --influx-org my-org \\');
  console.log('    --influx-bucket test-results \\');
  console.log('    --influx-token YOUR_TOKEN \\');
  console.log('    --runner-name "GitHub Actions" \\');
  console.log('    --run-id "${{ github.run_id }}"');
  process.exit(1);
}

const config = parseArgs(args);
const inputFile = config.inputFile;

// Validate config only if not dry run
if (!config.dryRun) {
  if (!config.influxConfig.url || !config.influxConfig.org || 
      !config.influxConfig.bucket || !config.influxConfig.token) {
    console.error('Error: Upload mode requires --influx-url, --influx-org, --influx-bucket, and --influx-token');
    console.error('\nTip: Use --dry-run to test without uploading');
    process.exit(1);
  }
}

// Main execution
(async () => {
  const points = await convertJUnitToInfluxPoints(inputFile, config.testRun);
  await uploadToInfluxDB(points, config.influxConfig, config.dryRun);
})();
