# Holochain Release process:

## Release candidate test

- Implement fixes, features, changes, and create a release candidate version in the format x.x.x-rc.x
- Compile a version of the ziptest application in the standalone kangaroo app runtime using this release candidate version
- Perform smoke test over 2 consecutive days:
  - Group of 3 testers
  - Day 1:
    - All nodes go online.
    - Each node sends 10 signals to each peer individually.
    - Each node sends 10 signals to "Everyone".
    - Each node creates 10 entries, 1 rep.
    - Wait for entries to appear for all participants.
    - One node goes offline.
    - Remaining nodes create another 10 entries, 1 rep.
    - Wait for entries to appear again.
    - Offline node goes online and waits for entries to appear.
    - After 15 minutes create another node and make sure it catches up. **15 minutes is the cutoff point for "what's new" to ring/disc sync**
    - One node keeps the conductor running over night and creates 10 entries while other peers are offline.
  - Day 2:
    - All nodes go online and await synchronization.
    - Repeat entry creation while all nodes are online.
    - Repeat one node goes offline procedure.
    - Create a new node and wait for it to synchronize.
- If smoke test passes, full release of the release candidate version is approved.
- If stress test fails, fix and update and release a new rc version and perform smoke test again.

## Full release test

- Once smoke test passes, and full version is released, bump versions in downstream components and re-publish happs, 
  and initiate full testing. This includes the demo apps Talking Stickies and Kando.
- Once testing passes and persistent DHT is functional for the given time period and no bugs have been reported, 
 elevate release to "recommended" status.

## Happy Path estimates:

- Estimated time from a release candidate to smoke test results: 2-3 days
- Estimated time from release candidate smoke test passed to full release: 1-3 days
- Estimated time from full release to tested stable release: 10-16 days (1-2 weeks to bump versions on all tools and test, plus 2 days testing)
