# Holochain Release process:

## Release candidate test

- Implement fixes, features, changes, and create a release candidate version in the format x.x.x-rc.x
- Compile a version of the ziptest application in the standalone kangaroo pouch single app runtime using this x.x.x-rc.x version
- Perform smoke test:
  - Add images under 50 KiB
  - Entries must be received by other peers
    - 0-3 second average reception time: timely (pass)
    - 3-10 second average reception time: delayed (warn)
    - 10+ second average reception time: unacceptable (fail)
    - Average reception time is estimated loosely based on 10 or more attempts in one or more sessions, measured with a stopwatch. "Update time" is defined as the time it takes for one node to "store" an image, and another node or nodes to register the hash for that stored image.
    - Smoke testing is not exhaustive, but an indicator that a release candidate and its dependencies compile, and that basic functionality is confirmed working.
- If smoke test passes, full release of the release candidate version is approved.
- If stress test fails, fix and update and release a new rc version and perform smoke test again.
- Once smoke test passes, and full version is released, bump versions in downstream components and re-publish happs, and initiate full testing. This includes the demo apps Talking Stickies and Kando.

## Full release test

- After release of full version, test again as before with the release candidate and also start a test of persistent DHT functionality that lasts approx. 2 weeks.
- Once testing passes and persistent DHT is functional for the given time period and no bugs have been reported, declare release as "stable".

## Happy Path estimates:

- Estimated time from a release candidate to smoke test results: 1-3 days
- Estimated time from release candidate smoke test passed to full release: 1-3 days
- Estimated time from full release to tested stable release: 3-4 weeks (1-2 weeks to bump versions on all tools and test, plus 2 weeks persistent DHT testing)
