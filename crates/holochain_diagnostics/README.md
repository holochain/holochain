# holochain_diagnostics

Tools and patterns to help write Holochain diagnostic tests.

This README represents a statement of intention that we hope to grow into, moreso than an actual state of being. We hope this crate will grow and become more useful over time.

## Diagnostic tests

A diagnostic test is an experiment which aims to unearth some new understanding about Holochain. The experiment may begin with some goal, or it may be an open-ended exploration. These tests are not meant to be run as part of our Continuous Integration workflow, though a diagnostic test may reveal a problem which can than be codified into a proper unit or integration test.

Some reasons to write a diagnostic test:

- Attempting to reproduce some problem out in the wild whose cause is unknown and seems to arise out of complex circumstances
- Investigating performance issues or generally trying to understand why the system is behaving the way it is
- Preemptively exploring possible failure modes: stress tests, fuzz testing, attack scenarios, etc.


## Tools

As we write diagnostic tests, we hope to identify common patterns that can be streamlined into reusable tools. We expect our tooling to provide:

- Configurable setup, which sets up some number of conductors with specified config and properties
- Reusable behavior "modules" which drive a conductor with some predefined behavior over a period of time, and which can be reused across tests.
    - "Behavior" includes zome definitions to describe *what* will happen, as well as some code which calls into those zomes, describing *how much* and *when* those functions will be called.
- Ability to easily reason about logs across multiple conductors
    - At the minimum, we need to be able to tell which conductor a line of logging is coming from
    - Eventually we hope to have tracing across multiple conductors
    