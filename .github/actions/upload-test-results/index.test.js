const { parseJUnitXML } = require('./index');
const fs = require('fs').promises;
const path = require('path');

describe('parseJUnitXML', () => {
  it('should parse sample JUnit XML correctly', async () => {
    const xmlPath = path.join(__dirname, 'sample-junit.xml');
    const xmlContent = await fs.readFile(xmlPath, 'utf8');
    
    const result = await parseJUnitXML(xmlContent);
    
    // Verify metadata
    expect(result.metadata).toBeDefined();
    expect(result.metadata.name).toBe('nextest-run');
    expect(result.metadata.totalTests).toBe(1289);
    expect(result.metadata.failures).toBe(0);
    expect(result.metadata.errors).toBe(0);
    expect(result.metadata.uuid).toBe('f444d7f5-95dd-4597-ba00-a8a81d114ad7');
    expect(result.metadata.timestamp).toBeInstanceOf(Date);
    expect(result.metadata.totalDuration).toBeCloseTo(385.322, 2);
    
    // Verify points array
    expect(result.points).toBeDefined();
    expect(result.points.length).toBeGreaterThan(0);
    
    // Verify statuses array
    expect(result.statuses).toBeDefined();
    expect(result.statuses.length).toBe(result.points.length);
    expect(result.statuses.every(s => ['passed', 'failed', 'flaky'].includes(s))).toBe(true);
    
    // Verify a sample point has expected structure
    const firstPoint = result.points[0];
    expect(firstPoint).toBeDefined();
    
    // Convert point to line protocol to verify structure
    const lineProtocol = firstPoint.toLineProtocol();
    expect(lineProtocol).toContain('test_result');
    expect(lineProtocol).toContain('test_suite=');
    expect(lineProtocol).toContain('test_name=');
    expect(lineProtocol).toContain('class_name=');
    expect(lineProtocol).toContain('status=');
    expect(lineProtocol).toContain('duration=');
    expect(lineProtocol).toContain('has_failure=');
    expect(lineProtocol).toContain('has_flaky_failure=');
    
    // Verify system-out and system-err are NOT included
    expect(lineProtocol).not.toContain('system_out=');
    expect(lineProtocol).not.toContain('system_err=');
  });
  
  it('should handle missing timestamps gracefully', async () => {
    const xmlContent = `<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="test-run" tests="1" failures="0" errors="0" uuid="test-uuid" time="1.0">
  <testsuite name="test-suite" tests="1" disabled="0" errors="0" failures="0">
    <testcase name="test-case" classname="test-class" time="1.0">
    </testcase>
  </testsuite>
</testsuites>`;
    
    const result = await parseJUnitXML(xmlContent);
    
    expect(result.metadata.timestamp).toBeInstanceOf(Date);
    expect(result.points.length).toBe(1);
    
    // Should use current time or suite timestamp as fallback
    const lineProtocol = result.points[0].toLineProtocol();
    expect(lineProtocol).toBeDefined();
  });
  
  it('should handle missing numeric fields gracefully', async () => {
    const xmlContent = `<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="test-run" tests="invalid" failures="NaN" errors="" uuid="test-uuid" timestamp="2025-01-01T00:00:00Z" time="invalid">
  <testsuite name="test-suite" tests="1" disabled="0" errors="0" failures="0">
    <testcase name="test-case" classname="test-class" timestamp="2025-01-01T00:00:00Z" time="invalid">
    </testcase>
  </testsuite>
</testsuites>`;
    
    const result = await parseJUnitXML(xmlContent);
    
    // Should fallback to 0 for invalid numbers
    expect(result.metadata.totalTests).toBe(0);
    expect(result.metadata.failures).toBe(0);
    expect(result.metadata.errors).toBe(0);
    expect(result.metadata.totalDuration).toBe(0.0);
    
    expect(result.points.length).toBe(1);
    const lineProtocol = result.points[0].toLineProtocol();
    expect(lineProtocol).toContain('duration=0');
  });
  
  it('should correctly identify test status', async () => {
    const xmlContent = `<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="test-run" tests="3" failures="1" errors="0" uuid="test-uuid" timestamp="2025-01-01T00:00:00Z" time="3.0">
  <testsuite name="test-suite" tests="3" disabled="0" errors="0" failures="1">
    <testcase name="passed-test" classname="test-class" timestamp="2025-01-01T00:00:00Z" time="1.0">
    </testcase>
    <testcase name="failed-test" classname="test-class" timestamp="2025-01-01T00:00:01Z" time="1.0">
      <failure message="Test failed" type="AssertionError">Stack trace here</failure>
    </testcase>
    <testcase name="flaky-test" classname="test-class" timestamp="2025-01-01T00:00:02Z" time="1.0">
      <flakyFailure message="Flaky failure" type="AssertionError" timestamp="2025-01-01T00:00:02Z" time="0.5">Flaky stack trace</flakyFailure>
    </testcase>
  </testsuite>
</testsuites>`;
    
    const result = await parseJUnitXML(xmlContent);
    
    expect(result.points.length).toBe(3);
    expect(result.statuses).toEqual(['passed', 'failed', 'flaky']);
    
    // Verify passed test
    const passedLine = result.points[0].toLineProtocol();
    expect(passedLine).toContain('status=passed');
    expect(passedLine).toContain('has_failure=0i');
    expect(passedLine).toContain('has_flaky_failure=0i');
    
    // Verify failed test
    const failedLine = result.points[1].toLineProtocol();
    expect(failedLine).toContain('status=failed');
    expect(failedLine).toContain('has_failure=1i');
    expect(failedLine).toContain('has_flaky_failure=0i');
    expect(failedLine).toContain('failure_message=');
    expect(failedLine).toContain('failure_type=\"AssertionError\"');
    
    // Verify flaky test
    const flakyLine = result.points[2].toLineProtocol();
    expect(flakyLine).toContain('status=flaky');
    expect(flakyLine).toContain('has_failure=0i');
    expect(flakyLine).toContain('has_flaky_failure=1i');
    expect(flakyLine).toContain('flaky_message=');
    expect(flakyLine).toContain('flaky_type=\"AssertionError\"');
    expect(flakyLine).toContain('flaky_duration=');
  });
});
