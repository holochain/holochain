#!/bin/bash
crates_to_document=("hdi" "hdk" "holochain_keystore" "holochain_state")

for crate in "${crates_to_document[@]}"; do
    echo 'generating README for crate' "$crate"
    cargo rdme -w $crate --intralinks-strip-links --force
done

# have any READMEs been updated?
git diff --exit-code --quiet
readmes_updated=$?
if [[ "$readmes_updated" == 1 ]]; then
    echo 'READMEs have been updated, committing changes'
    git config --local user.name release-ci
    git config --local user.email ci@holo.host
    git commit -am "docs(crate-level): generate readmes from doc comments"
    git config --local --unset user.name
    git config --local --unset user.email
fi
