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
