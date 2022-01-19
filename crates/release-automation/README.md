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

The release workflow interacts with these three repositories in the holochain organization:

  - ***holochain***: holochain core applications source code.
  - ***holochain-nixpkgs***: Nix package definitions for the holochain core applications and related tools.
  - ***holonix***: Nix shell definitions for coherent development environments for both holochain core developers as well as holochain application developers.

### Instructions

The following instructions are a work-in-progress.
_[M]anual_ and _[A]utomated_ are given on each step to indicate manual or automated steps.
Automated steps still require running the tool manually ;-).

0. _[M]_ Decide it's the time for a new release.
   Run a terminal in the root of the repository.
   Make sure you're on the commit from which you want to release.

0. _[M]_ Open a terminal and prepare the environment:

    Store all the variables in a file:

    ```sh
    cat <<EOF > ~/.holochain_release.sh
    export HOLOCHAIN_URL="git@github.com:holochain/holochain.git"
    export HOLOCHAIN_NIXPKGS_URL="git@github.com:holochain/holochain-nixkpgs.git"
    export HOLONIX_URL="git@github.com:holochain/holonix.git"

    export HOLOCHAIN_REPO=$(mktemp -d)
    export HOLOCHAIN_NIXPKGS_REPO=$(mktemp -d)
    export HOLONIX_REPO=$(mktemp -d)
    EOF
    ```

    Load the environment variables from the file into the current shell:


    ```sh
    source ~/.holochain_release.sh
    ```

    If you want to run a second terminal or you loose your shell for some reason, you can use this command to load the variables.

0. _[M]_ Prepare the holochain repository and enter it.

    ```sh
    git clone "${HOLOCHAIN_URL}" "${HOLOCHAIN_REPO}"
    pushd "${HOLOCHAIN_REPO}"
    ```

0. _[A]_ Create the release branch and bump the versions.

    In detail:

    0. Create a new release branch from develop
    0. For all changed crates, bump their version to a develop version.
    0. For the main crates and all of their dependencies in the workspace:
       - Include candidates by all of these positive indicators:
           * they have changed since their last release OR they haven't had a release
           * version number is allowed by a the given requirement
       - Exclude candidates by any of these negative indicators:
           * CHANGELOG.md contains `unreleaseable: true` in its front matter
           * version number is disallowed by a requirement
           * description or license fields are missing
    0. Increase the package version in each Cargo.toml file to the desired release level
    0. Rotate the unreleased heading content to a release heading in each crate's CHANGELOG.md file
    0. Add a workspace release heading in the workspace CHANGELOG.md file with the aggregated content of all included releases
    0. Create a commit with the version and changelog changes
    0. Create a tag for each crate release (***NOTE***: This is likely subject to change because it creates issues in case of publish failures later on. It would probably be preferable to only create tags after a successful publish.)

    The commands for this are:

    ```sh
    nix-shell --run '
      hc-ra \
        --workspace-path=$PWD \
        --log-level=info \
        release \
          --steps=CreateReleaseBranch
      '

    nix-shell --run '
      hc-ra \
        --workspace-path=$PWD \
        crate apply-dev-versions --commit
      '

    nix-shell --run '
      hc-ra \
        --workspace-path=$PWD \
        --log-level=info \
        release \
          --dry-run \
          --match-filter="^(holochain|holochain_cli|kitsune_p2p_proxy)$" \
          --disallowed-version-reqs=">=0.1" \
          --steps=BumpReleaseVersions
      '
    ```

    If this succeeds, repeat the command without the `--dry-run` to perform the changes.

    ***NOTE***: If at any point after this any changes need to be made to the code for this release, please come back here and follow these steps:

    0. Drop the commit made by the _BumpReleaseVersions_ step using `git rebase -i HEAD~1`.
    0. Make the required changes and commit them.
    0. Repeat the _BumpReleaseVersions_ step.
    0. Continue the process from there...

0. _[M]_ Add additional release-specific environment variables:

    Conditionally, if you're running this step from a new shell, populate the environment variables again and navigate to the holochain repository.

    ```sh
    source ~/.holochain_release.sh
    pushd ${HOLOCHAIN_REPO}
    ```

    The following syntax ensures the variables are currently available:

    ```sh
    cat <<EOF > ~/.holochain_release.sh
    export HOLOCHAIN_URL=${HOLOCHAIN_URL:?}
    export HOLOCHAIN_NIXPKGS_URL=${HOLOCHAIN_NIXPKGS_URL:?}
    export HOLONIX_URL=${HOLONIX_URL:?}

    export HOLOCHAIN_REPO=${HOLOCHAIN_REPO:?}
    export HOLOCHAIN_NIXPKGS_REPO=${HOLOCHAIN_NIXPKGS_REPO:?}
    export HOLONIX_REPO=${HOLONIX_REPO:?}

    export TAG=$(git tag --list | grep holochain- | tail -n1)
    export VERSION=${TAG/holochain-/}
    export VERSION_COMPAT="v${VERSION//./_}"
    export RELEASE_BRANCH=$(git branch --show-current)
    EOF
    ```

    Source the environment file once more:

    ```
    source ~/.holochain_release.sh
    ```

0. _[M]_ Push the release branch. Example:

    ```sh
    git push -u origin $(git branch --show-current)
    ```

0. _[M]_ Create a Pull-Request from the release branch to the main branch and ensure the CI tests pass

0. _[M]_ Ensure release branch is fast-forward mergable to the main branch.
    Example:

    ```sh
    git fetch origin
    git checkout -B main-merge-test origin/main
    git merge --ff-only "${RELEASE_BRANCH}"
    git checkout "${RELEASE_BRANCH}"
    git branch -D main-merge-test
    ```

0. _[M]_ Test that holochain-nixpkgs and holonix don't break.

    ***NOTE: best effort steps, might not work verbatim yet***

    0. Push the most recent holochain tag.

        ```sh
        git push origin "${TAG}"
        ```

    0. Add an entry for this holochain tag to holochain-nixpkgs' _update\_config.toml_

        ```sh
        git clone "${HOLOCHAIN_NIXPKGS_URL}" "${HOLOCHAIN_NIXPKGS_REPO}"
        pushd "${HOLOCHAIN_NIXPKGS_REPO}"

        git checkout -b "${RELEASE_BRANCH}"

        cat <<EOF >> packages/holochain/versions/update_config.toml

        [${VERSION_COMPAT}]
        git-src = "revision:${TAG}"
        lair-version-req = "~0.0"
        EOF

        # add the new tag to the CI config so it's built and cached by it
        nix-shell -p yq-go --run 'yq e -i \
          ".jobs.holochain-binaries.strategy.matrix.nixAttribute = .jobs.holochain-binaries.strategy.matrix.nixAttribute + [\"${VERSION_COMPAT}\"]" \
          .github/workflows/build.yml'

        git commit -m "add ${TAG}" .github/workflows/build.yml packages/holochain/versions/update_config.toml

        # regenerate the nix sources
        nix-shell --arg flavors '["release"]' --pure --run "hnixpkgs-update-single ${VERSION_COMPAT}"

        git push origin ${RELEASE_BRANCH}
        ```

    0. Create a PR on holochain-nixpkgs and wait for CI to succeed.

    0. Create a new branch in the holonix repo and point holonix it to the `${RELEASE_BRANCH}` of holochain-nixpkgs to test its changes.

        ```sh
        git clone "${HOLONIX_URL}" "${HOLONIX_REPO}"
        pushd "${HOLONIX_REPO}"

        git fetch origin

        git checkout -b "${RELEASE_BRANCH}" origin/develop

        nix-shell -p niv --run "niv modify holochain-nixpkgs -b ${RELEASE_BRANCH}"

        ./nix/update.sh

        nix-shell --run hn-test --argstr holochainVersionId "${VERSION_COMPAT}"

        git push origin "${RELEASE_BRANCH}"
        ```

    0. Create a PR on holonix and wait for CI to succeed.

0. _[M]_ Merge the holochain/holochain release branch into the main branch.

    In the holochain repo, do:

    ```sh
    pushd ${HOLOCHAIN_REPO}

    git checkout main
    git pull origin main
    git merge --ff-only "${RELEASE_BRANCH}"
    ```

0. _[A]_ Publish all the bumped crates to crates.io.

    0. Run a variation of `cargo publish --dry-run` for all bumped crates.
       Expected errors, such as missing dependencies of new crate versions, will be detected and tolerated.

        ```sh
        nix-shell --pure --run '
          hc-ra \
            --workspace-path=$PWD \
            --log-level=debug \
            release \
              --dry-run \
              --steps=PublishToCratesIo
          '
        ```

        If this succeeds, repeat the command without the `--dry-run` to perform the changes.

    0. Ensure the *(FIXME: hardcoded)* set of owners are invited to all crates.

        ```sh
        nix-shell --pure --run '
          hc-ra \
            --workspace-path=$PWD \
            --log-level=info \
            release \
              --dry-run \
              --steps=AddOwnersToCratesIo \
          '
        ```

        If this succeeds, repeat the command without the `--dry-run` to perform the changes.

0. _[M]_ Push the holochain/holochain main branch and the new tags upstream.

    In the holochain repo, do:

    ```sh
    git push origin main --tags
    ```

0. _[M]_ Merge the holochain develop branch into the release branch in case it has advanced in the meantime. Example:

    ```sh
    git checkout "${RELEASE_BRANCH}"
    git fetch origin
    git merge origin/develop
    git push origin "${RELEASE_BRANCH}"
    ```

0. _[M]_ Create and merge a PR from the holochain/holochain release branch to the develop branch, wait for CI to pass and merge it.

0. _[M]_ Update and merge the PR on holochain/holochain-nixpkgs.

    In the holochain-nixpkgs repo:

    ```sh
    pushd "${HOLOCHAIN_NIXPKGS_REPO}"
    nix-shell --arg flavors '["release"]' --pure --run "hnixpkgs-update-single main"
    nix-shell --arg flavors '["release"]' --pure --run "hnixpkgs-update-single develop"
    git push origin ${RELEASE_BRANCH}
    ```

    Wait for the CI tests to pass and merge it.

0. _[M]_ Update and merge the PR on holochain/holonix to the develop ***and*** main branch from the same source.

    In the holonix repo:

    ```sh
    pushd ${HOLONIX_REPO}
    nix-shell -p niv --run "niv modify holochain-nixpkgs -b develop"
    ./nix/update.sh
    git push origin "${RELEASE_BRANCH}"
    ```

    Wait for CI to pass and merge both PRs.

0. _[M]_ Cleanup the temporary directories.

    ```sh
    rm -rf ${HOLOCHAIN_REPO} ${HOLOCHAIN_NIXPKGS_REPO} ${HOLONIX_REPO} ~/.holochain_release.sh
    ```

0. _[M]_ [Draft and create a GitHub release](https://github.com/holochain/holochain/releases/new) from the new holochain tag on holochain/holochain.
    0. Select the new holochain tag via the _Choose a tag_ button.

    0. Choose the title: _Holochain <VERSION>_

    0. Set the description according to this template:

        ```md
        <INSERT HOLOCHAIN CHANGELOG ENTRIES>

        ---

        ***Please read [this release's top-level CHANGELOG](https://github.com/holochain/holochain/blob/main/CHANGELOG.md#<RELEASE_TIMESTAMP>) for details and changes in all crates in this repo.***
        ```
    0. Push the _Publish release_ button \o/

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
