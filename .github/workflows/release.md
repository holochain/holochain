# release.yml

## Contained Jobs

- vars: processes the input
- prepare: imported from ./release-prepare.yml
- test: currently unimplemented
- finalize:
  - restore repo from cache
  - push to the target branch
  - push to the release branch (TODO: What is the difference)
  - publish crates
  - push tags
  - merge release branch into source branch
  - push updated source branch
  - create a pull-request towards the source branch
  - create a github release
