#!/usr/bin/env bash
set -eux

# a positive condition means the current holochain version has already been released, hence this release doesn't contain holochain
if gh release view "${LATEST_HOLOCHAIN_TAG}"; then
  export RELEASE_TAG=${RELEASE_BRANCH}
  export IS_HOLOCHAIN_RELEASE="false"
else
  export RELEASE_TAG=${LATEST_HOLOCHAIN_TAG}
  export IS_HOLOCHAIN_RELEASE="true"
fi

# this configuers a single, hardcoded branch to publish "latest" release's
# this makes the release stand out on the right side in the github repository landing page
if [[ "${HOLOCHAIN_TARGET_BRANCH}" == "main-0.1" && "${IS_HOLOCHAIN_RELEASE}" == "true" ]]; then
  export IS_LATEST="true"
else
  export IS_LATEST="false"
fi

# simply check for the delimeter between the version number and a pre-release suffix
if [[ "${LATEST_HOLOCHAIN_VERSION}" == *"-"* ]]; then
  export IS_PRE_RELEASE="true"
else
  export IS_PRE_RELEASE="false"
fi

cmd=(
   gh api
   --method POST
   /repos/holochain/holochain/releases
   -H "Accept: application/vnd.github+json"
   -f tag_name="${RELEASE_TAG}"
   -f target_commitish="${HOLOCHAIN_TARGET_BRANCH}"
   -f name="holochain ${LATEST_HOLOCHAIN_VERSION} (${RELEASE_BRANCH#*-})"
   -f body="***Please read [this release's top-level CHANGELOG](https://github.com/holochain/holochain/blob/${HOLOCHAIN_TARGET_BRANCH}/CHANGELOG.md#$(sed -E 's/(release-|\.)//g' <<<"${RELEASE_BRANCH}")) to see the full list of crates that were released together.***" \
   -F draft=false
   -F generate_release_notes=false
   -F prerelease="${IS_PRE_RELEASE}"
   -f make_latest="${IS_LATEST}"
)

if [[ "${DRY_RUN:-true}" == "false" ]]; then
    "${cmd[@]}"
else
    echo "${cmd[@]}"
fi
