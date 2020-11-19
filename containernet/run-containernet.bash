#!/bin/bash

set -eEuo pipefail

cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")"

trap "vagrant halt || true" EXIT

vagrant up --provider virtualbox
vagrant ssh-config > ssh_config
ssh -F ./ssh_config default 'sudo bash -l /holochain/containernet/guest_run.bash'
