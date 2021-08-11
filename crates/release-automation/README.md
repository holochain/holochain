# release-automation

This project codifies Holochain's opinionated release workflow.
It supports selectively releasing crates within a [cargo workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html) with flexible handling of release blockers.
The aim is to build a CLI tool that can be used manually and also within the context of CI for fully automated releases.

## Status

*Prematurity Warning: **everything** in here might be **subject to change.***

This tool currently has many hardcoded names and opinions that are specific to this repository structure.
It's thus not likely to work in an unrelated project.

### Related projects and rationale

It would be nice to eventually consolidate this project with an already existing project with enough flexibility to cover the union of the supported use-cases. These projects are similar and could potentially be united this project:

* [cargo-release](https://github.com/sunng87/cargo-release): there was an attempt to use a modified version of this but the opionions on the desired workflow currently suggest to build it out from scratch.
* [cargo-workspaces](https://github.com/pksunkara/cargo-workspaces):
* [cargo-mono](https://github.com/kdy1/cargo-mono)
* [unleash](https://github.com/tetcoin/unleash)

There's a related issue on cargo tracking: [cargo publish multiplate packages at once](https://github.com/rust-lang/cargo/issues/1169).

## Repository Requirements

* Toplevel _Cargo.toml_ manifest with a `[workspace]` declaration
* Toplevel _CHANGELOG.md_ file
* Member crates in the _crates_ directory with a valid `Cargo.toml` manifest with a `[package]` declaration
* One _CHANGELOG.md_ file per crate

## Installation

From the root of this repository simply run `nix-shell`. This will make the `hc-ra` command available.

## Workflow

The workflow is split up into multiple steps that involve different branches.

Each release involves three branches:
- **develop**: this is where development takes place on a day to day bases.
- **release-YYYYMMDD.HHMMSS**: for each release _develop_ is branched off into a new release branch with this naming scheme.
- **main**: release branches are merged into this for downstream consumption.

### Instructions

The following instructions are a work-in-progress.
_[M]anual_ and _[A]utomated_ are given on each step to indicate manual or automated steps.
Automated steps still require running the tool manually ;-).

0. _[M]_ Decide it's the time for a new release.
   Run a terminal in the root of the repository.
   Make sure you're on the commit from which you want to release.

0. _[A]_ Create the release branch and bump the versions. In detail:

    0. Create a new release branch from develop
    0. For the main crates and all of their dependencies in the workspace:
       - Include candidates by all of these positive indicators:
           * they have changed since their last release OR they haven't had a release
           * version number is allowed by a the given requirement
       - Exclude candidates by any of these negative indicators:
          * CHANGELOG.md contains `unreleaseable = true` in its front matter
           * version number is disallowed by a requirement
           * description or license fields are missing
    0. Increase the package version in each Cargo.toml file to the desired release level
    0. Rotate the unreleased heading content to a release heading in each crate's CHANGELOG.md file
    0. Add a workspace release heading in the workspace CHANGELOG.md file with the aggregated content of all included releases
    0. Create a commit with the version and changelog changes
    0. Create a tag for each crate release (***NOTE***: This is likely subject to change because it creates issues in case of publish failures later on. It would probably be preferable to only create tags after a successful publish.)

    The command for this is:

    ```sh
    hc-ra \
        --log-level=info \
        release \
            --dry-run \
            --match-filter="^(holochain|holochain_cli|kitsune_p2p_proxy)$" \
            --disallowed-version-reqs=">=0.1" \
            --allowed-matched-blockers=UnreleasableViaChangelogFrontmatter \
            --steps=CreateReleaseBranch,BumpReleaseVersions'
    ```

    If this succeeds, repeat the command without the `--dry-run` to perform the changes.

0. _[M]_ Push the release branch. Example:

    ```sh
    git push -u upstream $(git branch --show-current)
    ```

0. _[M]_ Create a Pull-Request from the release branch to the main branch

0. _[M]_ Ensure the CI tests pass

0. _[M]_ Ensure release branch is fast-forward mergable to the main branch
    Example:

    ```sh
    export RELEASE_BRANCH=$(git branch --show-current)
    git checkout -B main-merge-test upstream/main
    git merge --ff-only "${RELEASE_BRANCH}"
    git checkout "${RELEASE_BRANCH}"
    git branch -D main-merge-test
    ```
0. _[T]_ Publish all the bumped crates to crates.io.

    0. Run a variation of `cargo publish --dry-run` for all bumped crates.
        Expected errors, such as missing dependencies of new crate versions, will be detected and tolerated.
    0. On successful publish, the tool will invite the missing owners according to the the given *(FIXME: hardcoded)* given set of owners and the ones who are set on the registry.

    ```sh
    hc-ra \
    --workspace-path=$PWD \
    --log-level=debug \
    release \
        --dry-run \
        --steps=PublishToCratesIo'
    ```

    If this succeeds, repeat the command without the `--dry-run` to perform the changes.

0. _[M]_ Push the tags. Example:
    ```sh
    git push upstream --tags
    ```

0. _[M]_ Merge the develop branch into the release branch if it has advanced in the meantime. Example:
    ```sh
    git fetch --all
    git merge develop $(git branch --show-current)
    git push upstream
    ```
0. _[M]_ Create and merge a PR from the release branch to develop

## Development

With the `nix-shell` you can run the test suite using:

```sh
nix-shell --run hc-release-automation-test
```

## Continuous Integration

A partial goal of this tool is to ensure the state of the repository remains releasable.
This can be achieved by configuring CI to run the tool with a variation of the `release --dry-run` subcommand.

***NOTE***: There is currently an uncertain issue in the workflow that pertains to whether and when to set the development versions, which has an influence on the `cargo publish` command and potentially other `cargo` subcommands.
This likely influences the reliability of the integration tests.
This is because the local version number of a crate appears to be published, even though there were changes since the crate was published.
