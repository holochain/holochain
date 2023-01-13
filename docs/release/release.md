# Release Workflow Guides

## Trigger A Release Manually

![release holochain workflow](./release-holochain_0.png)

This requires you have GitHub credentials with appropriate permissions.

1. Visit https://github.com/holochain/holochain/actions
2. Select the "release holochain" workflow
3. Press the "Run workflow" button
4. Indicate whether this is a dry-run (keep _true_) or a real release (change the field to _false_)
5. Optional: to set up a debug SSH session for debugging failure scenarios change the field to _true_
6. Confirm by clicking on "Run workflow"

## (Permanently) Marking A Crate for Major/Minor/Patch/Pre Version Bumps

The _release-automation_ tool parses **each crate**'s _CHANGELOG.md_ file to read these two attributes from the frontmatter:

* `semver_increment_mode`

    This attribute will be removed after each successful release, and can thus be used as a one-time (per-crate) instruction.
* `default_semver_increment_mode`

    This attribute will be retained, and can thus be used to define a permanent (per-crate) configuration.

If both of them are missing from the frontmatter, [_patch_ is used as the default](https://github.com/holochain/holochain/blob/bc621e3e06e998d35750b2bac6b0e1f0d371c2a2/crates/release-automation/src/lib/common.rs#L150-L154).

For both, this is the complete list of valid variants:
* _major_
* _minor_
* _patch_
* _!pre \<pre-release-suffix\>_ (e.g. `!pre dev`)
* _!pre\_major \<pre-release-suffix\>_ (e.g. `!pre_patch rc`)
* _!pre\_minor \<pre-release-suffix\>_ (e.g. `!pre_patch beta`)
* _!pre\_patch \<pre-release-suffix\>_ (e.g. `!pre_patch alpha`)

**The exclamation mark is required for the values that take a pre-release-suffix**, as the parser relies on [YAML tags for explicit type hints](https://yaml.org/spec/1.2.2/#tags).*

### Syntax
The frontmatter is parsed as YAML and expects a `key: value` attribute format.

Example:

```markdown
---
semver_increment_mode: !pre\_minor "rc"
---

# Changelog

...
```

### Precedence

They interact in the following way:

`semver_increment_mode` | `default_semver_increment_mode` | Version Outcome
--- | --- | ---
not given | not given | fallback to _patch_
not given | given | $default_semver_increment_mode
given | *ignored* | $semver_increment_mode

### Pre-Release-Suffix Handling
For any of the _pre_ modes, if at the time of release a pre-release suffix is found in the version, the outcome depends on the existing suffix:
* if the version **does not already** have a pre-release suffix: bump version according to the requested level, and append `-<pre-release-suffix>.0`
* if the version **does already** have a pre-release suffix:
    * if the **existing suffix is the same** as the requested one:
        * if **it is followed** by a dot and an integer: the integer will be incremented by 1
        * if **it is not followed** by a dot and an integer: ".0" will be added to the suffix
    * if the **existing suffix is different** than the requested one: replace it with `-<pre-release-suffix>.0`

### Artifical examples of consecutive releases

For an almost exhaustive list of tested transition cases look at the `fn increment_semver_consistency` test in [../../crates/release-automation/src/lib/common.rs](../../crates/release-automation/src/lib/common.rs).

#### Setting without and with `default_`

The _pre-release-suffix_ pertains no special meaning and is parsed as an arbitrary string.
However, it will have an incremental number >= 0 maintained on each consecutive release within the same pre-release-suffix.

Without `default_`:

* _0.0.1_
    * setting `semver_increment_mode: !pre_patch lorem` in the changelog here
    * the tooling will remove the setting in the changelog in the release process, and subsequently default back to `patch`
* _0.0.2-lorem.0_
* _0.0.2_
* _0.0.3_

With

* _0.0.1_
    * setting `default_semver_increment_mode: !pre_patch lorem` here
* _0.0.2-lorem.0_
* _0.0.2-lorem.1_
* _0.0.2-lorem.2_

### Real world example: hdi 0.1 minor bump

1. Before the next release: [hdi: mark for minor version bump #1550](https://github.com/holochain/holochain/pull/1550/commits)

    A developer proposed a PR for the `develop` branch based upon the decision to bump the hdi's minor version.
    The following shows the diff of the PR.

    1. It adjusts the release-prepare workflow's settings to allow for the resulting versions.
    2. It adds a frontmatter to the hdi's _CHANGELOG.md_ setting the attribute `semver_increment_mode: minor`.

    ```diff
    diff --git a/.github/workflows/release-prepare.yml b/.github/workflows/release-prepare.yml
    index 8c43f29b69..a1693df80e 100644
    --- a/.github/workflows/release-prepare.yml
    +++ b/.github/workflows/release-prepare.yml
    @@ -226,7 +226,7 @@ jobs:
                    --no-verify-pre \
                    --force-tag-creation \
                    --match-filter="^(holochain|holochain_cli|kitsune_p2p_proxy)$" \
    -                --disallowed-version-reqs=">=0.1" \
    +                --disallowed-version-reqs=">=0.2" \
                    --steps=BumpReleaseVersions

                cargo sweep -f
    diff --git a/crates/hdi/CHANGELOG.md b/crates/hdi/CHANGELOG.md
    index 60262e972c..482ccc678e 100644
    --- a/crates/hdi/CHANGELOG.md
    +++ b/crates/hdi/CHANGELOG.md
    @@ -1,8 +1,13 @@
    +---
    +semver_increment_mode: minor
    +---
    +
    # Changelog

    The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

    ## Unreleased
    +- Initial minor version bump. This indicates our impression that we have made significant progress towards stabilizing the detereministic integrity layer's API. [\#1550](https://github.com/holochain/holochain/pull/1550)

    ## 0.0.21
    ```

2. The next time a release is triggered on the develop branch (which is the default), the `release-automation` will consider the attribute for the hdi crate.

    In this case, the [following shows a snippet of the version bump commit that was produced](https://github.com/holochain/holochain/pull/1561/commits/1a291fb210f5e9e506339721f3a8a9d5760f3af6):

    ```diff
    diff --git a/crates/hdi/CHANGELOG.md b/crates/hdi/CHANGELOG.md
    index 482ccc678e..475626926e 100644
    --- a/crates/hdi/CHANGELOG.md
    +++ b/crates/hdi/CHANGELOG.md
    @@ -1,13 +1,12 @@
    ----
    -semver_increment_mode: minor
    ----
    -
    # Changelog

    The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

    ## Unreleased
    -- Initial minor version bump. This indicates our impression that we have made significant progress towards stabilizing the detereministic integrity layer's API. [\#1550](https://github.com/holochain/holochain/pull/1550)
    +
    +## 0.1.0
    +
    +- Initial minor version bump. This indicates our impression that we have made significant progress towards stabilizing the detereministic integrity layer’s API. [\#1550](https://github.com/holochain/holochain/pull/1550)

    diff --git a/crates/hdi/Cargo.toml b/crates/hdi/Cargo.toml
    index b4bf947dc8..7262cfbb3a 100644
    --- a/crates/hdi/Cargo.toml
    +++ b/crates/hdi/Cargo.toml
    @@ -1,6 +1,6 @@
    [package]
    name = "hdi"
    -version = "0.0.22-dev.0"
    +version = "0.1.0"
    ```

    Note that the release process removed the `semver_increment_mode` attribute so that it doesn't affect the next release.

3. Post-release: [Merge release-20220907.100911 back into develop #1561](https://github.com/holochain/holochain/pull/1561)

    This PR was created automatically by the release process to merge the release changes back into the _develop_ branch.


## Changing multiple frontmatters at once

When we plan to change the versions of many or all workspace crates, there's a command that can be used to overwrite the frontmatter of multiple crates' changelogs in one go:

```console
nix-shell --pure --argstr flavor release --run 'release-automation --workspace-path=$PWD --log-level=debug --match-filter=".*" changelog set-frontmatter <(cat <<EOF
(...)
EOF
)
'
```

The `--match-filter` argument takes a regular expression to select to filter the crate names.
The ellipsis give the position of the new YAML code for the frontmatters.

### Example: initiate a beta-rc cycle


```console
nix-shell --pure --argstr flavor release --run 'release-automation --workspace-path=$PWD --log-level=debug --match-filter=".*" changelog set-frontmatter <(cat <<EOF
default_semver_increment_mode: !pre_minor beta-rc
EOF
)
'
```

### Example: initiate a one-time minor version bump

```console
nix-shell --pure --argstr flavor release --run 'release-automation --workspace-path=$PWD --log-level=debug --match-filter=".*" changelog set-frontmatter <(cat <<EOF
semver_increment_mode: !minor
EOF
)
'
```
