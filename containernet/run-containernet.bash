#!/bin/bash

# sane bash errors
set -eEuo pipefail

# cd to script directory
cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")"

# print to stderr
eprintf() { printf "${@}" >&2; }

# print help/usage info and exit
help() {
  eprintf "\n"
  eprintf " # usage './run-containernet.bash'\n"
  eprintf "   execute containernet vm and test script\n\n"
  eprintf " # options\n"
  eprintf "   -h --help       | print this help info and exit\n"
  eprintf "   -n --no-halt    | do not halt vm after test run\n"
  eprintf "                   | (you can manually 'vagrant halt')\n"
  exit 127
}

_halt="1" # default to shutting down the vm
_help=""  # if set, print help/usage info and exit

# parse command-line options
while (( "${#}" )); do
  case "${1}" in
    -h|--help)
      _help=1
      shift
      ;;
    -n|--no-halt)
      _halt=""
      shift
      ;;
    *)
      eprintf "invalid option '%s'\n" "${1}"
      help
      ;;
  esac
done

# check if we should print help/usage info / exit
if [ "${_help}" == "1" ]; then
  help
fi

# check if we should setup a halt trap for the vm
if [ "${_halt}" == "1" ]; then
  trap "vagrant halt || true" EXIT
fi

# bring up the containernet VM
vagrant up --provider virtualbox

# get ssh config for accessing the VM
vagrant ssh-config > ssh_config

# execute our guest script on the VM
ssh -F ./ssh_config default 'sudo bash -l /holochain/containernet/guest_run.bash'
