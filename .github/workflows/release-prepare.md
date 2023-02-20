# release-prepare.yml

Loaded as part for `release.yml`  
Contains a single job called `prepare`.

As input it receives a branch to be released (aka. source branch).
It then merges the source branch into the release branch (usually `release`).

The merged state of the repo is then serialized and cached to use in the subsequent `release.yml` workflow.

This workflow also caches cargo related state and build files between runs.


## Relevant Steps

1. Merge the source branch into the release branch
1. Restore holochain cargo related state and build files
1. (Checks files) Detect missing release headings
1. (Changes files) Generate crate READMEs from doc comments
1. (Changes files) Bump the crate versions for the release
1. Cache the repo and set outputs
