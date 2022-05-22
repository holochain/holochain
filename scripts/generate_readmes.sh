#!/bin/sh

pushd crates/hdk > /dev/null
cargo readme -o README.md
popd > /dev/null

# have any READMEs been updated?
git diff --exit-code --quiet
readmes_updated=$?
if [[ "$readmes_updated" == 1 ]]; then
    git commit -am "docs(github): generated READMEs from doc comments"
fi