/*!
# Release Automation

This project codifies Holochain's opinionated release workflow.
It supports selectively releasing crates within a [cargo workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html) with flexible handling of release blockers.
The aim is to build a CLI tool that can be used manually and also within the context of CI for fully automated releases.

## Workflow

The workflow is split up into multiple steps that involve different branches.

Each release involves three branches:
- **develop**: this is where development takes place on a day to day bases.
- **release-YYYYMMDD.HHMMSS**: for each release _develop_ is branched off into a new release branch with this naming scheme.
- **main**: release branches are merged into this for downstream consumption.

### Brief summary

0. Decide it's the time for a new release
1. Create a new release branch from develop
2. For the main crates and all of there dependencies in the workspace:
    - Determine candidates by all of the positive indicators signaling:
        * they have changed since their last release by looking at their CHANGELOG.md OR they haven't had a release
        * version number is allowed by a the requirement
    - Skip candidates by any of these negative indicators signalling:
        * CHANGELOG.md contains `unreleaseable = true` in its front matter
        * version number is disallowed by a requirement
3. Increase the package version in each Cargo.toml file
4. Add a release in each CHANGELOG.md file
5. Add a workspace release in the workspace CHANGELOG.md file
6. Create a tag for each crate version
7. Create a PR from the release branch to the main branch
8. Merge PR to main
9. Publish crates to crates.io
10. Push the tags upstream
11. On the release branch increase the versions of all released crates to the next patch and develop version
12. Create a tag for the workspace release
13. Create and merge a PR to develop
14. Push the tags upstream

## Related projects and rationale

There was an attempt to use a modified version of [cargo-release](https://github.com/sunng87/cargo-release) but the opionions on the desired workflow currently suggest to build it out from scratch.
It would be nice to eventually consolidate both into a common project with enough flexibility to cover the union of the supported use-cases.

## Development

With the `nix-shell` you can run the test suite using:

```shell
nix-shell --run hc-release-automation-test
```
*/

#![allow(unused_imports)]
#![allow(dead_code)]

mod lib;
use lib::*;

use log::debug;
use structopt::StructOpt;

fn main() -> CommandResult {
    let args = cli::Args::from_args();

    env_logger::builder()
        .filter_level(args.log_level.to_level_filter())
        .filter(Some("cargo::core::workspace"), log::LevelFilter::Error)
        .parse_filters(&args.log_filters)
        .format_timestamp(None)
        .init();

    debug!("args: {:#?}", args);

    match &args.cmd {
        cli::Commands::Changelog(cmd_args) => crate::changelog::cmd(&args, cmd_args),
        cli::Commands::Check(cmd_args) => crate::check::cmd(&args, cmd_args),
        cli::Commands::Release(cmd_args) => crate::release::cmd(&args, cmd_args),
        cli::Commands::Crate(cmd_args) => crate::crate_::cmd(&args, cmd_args),
    }
}
