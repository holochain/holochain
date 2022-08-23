#!/bin/bash

crates_to_document=("hdk" "holochain_keystore" "holochain_state")

for crate in "${crates_to_document[@]}"; do
    echo 'generating README for crate' "$crate";
    cargo readme -r crates/"$crate" -o README.md
done

# have any READMEs been updated?
git diff --exit-code --quiet
readmes_updated=$?
if [[ "$readmes_updated" == 1 ]]; then
    echo 'READMEs have been updated, committing changes'
    git config user.name release-ci
    git config user.email ci@holo.host
    git commit -am "docs(crate-level): generate READMEs from doc comments"
fi