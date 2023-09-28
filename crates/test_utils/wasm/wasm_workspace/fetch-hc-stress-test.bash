#!/usr/bin/bash

rm -rf hc-stress-test-integrity
mkdir hc-stress-test-integrity
(cd hc-stress-test-integrity && curl -L https://github.com/matthme/hc-stress-test/tarball/hc-0.2.2-beta-rc.1 | tar xzf - --wildcards --strip-components=6 */dnas/files/zomes/integrity/files)

rm -rf hc-stress-test-coordinator
mkdir hc-stress-test-coordinator
(cd hc-stress-test-coordinator && curl -L https://github.com/matthme/hc-stress-test/tarball/hc-0.2.2-beta-rc.1 | tar xzf - --wildcards --strip-components=6 */dnas/files/zomes/coordinator/files)
