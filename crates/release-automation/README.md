# release-automation

## Release Automation

This project codifies Holochain's opinionated release workflow.
It supports selectively releasing crates within a [cargo workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html) with flexible handling of release blockers.
The aim is to build a CLI tool that can be used manually and also within the context of CI for fully automated releases.

### Workflow

The workflow is split up into multiple steps that involve different branches.

Each release involves three branches:
- **develop**: this is where development takes place on a day to day bases.
- **release-YYYYMMDD.HHMMSS**: for each release _develop_ is branched off into a new release branch with this naming scheme.
- **main**: release branches are merged into this for downstream consumption.

#### Brief summary

This is a brief event summary for the release workflow that is supported and (partially) automated by this project.

0. Decide it's the time for a new release
0. Create a new release branch from develop
0. For the main crates and all of their dependencies in the workspace:
    - Include candidates by all of these positive indicators:
        * they have changed since their last release OR they haven't had a release
        * version number is allowed by a the given requirement
    - Exclude candidates by any of these negative indicators:
        * CHANGELOG.md contains `unreleaseable = true` in its front matter
        * version number is disallowed by a requirement
0. Increase the package version in each Cargo.toml file to the desired release level
0. Rotate the unreleased heading content to a release heading in each crate's CHANGELOG.md file
0. Add a workspace release heading in the workspace CHANGELOG.md file with the aggregated content of all included releases
0. Create a single commit with the version and changelog changes and push it
0. Create a Pull-Request from the release branch to the main branch
0. Ensure the CI tests pass and the release branch is fast-forward mergable to the main branch
0. Publish the crates to crates.io
0. Create a tag for every released crate and push it
0. On the release branch increase the versions of all released crates to the next patch and develop version
0. Create a single commit for these version changes and push it
0. Create a tag for the workspace release
0. Merge the develop branch into the release branch if it has advanced in the meantime
0. Create and merge a PR from the release branch to develop
0. Push the workspace tag

### Related projects and rationale

It would be nice to eventually consolidate this project with an already existing project with enough flexibility to cover the union of the supported use-cases. These projects are similar and could potentially be united this project:

* [cargo-release](https://github.com/sunng87/cargo-release): there was an attempt to use a modified version of this but the opionions on the desired workflow currently suggest to build it out from scratch.
* [cargo-workspaces](https://github.com/pksunkara/cargo-workspaces):
* [cargo-mono](https://github.com/kdy1/cargo-mono)
* [unleash](https://github.com/tetcoin/unleash)

There's a related issue on cargo tracking: [cargo publish multiplate packages at once](https://github.com/rust-lang/cargo/issues/1169).

### Development

With the `nix-shell` you can run the test suite using:

```shell
nix-shell --run hc-release-automation-test
```
