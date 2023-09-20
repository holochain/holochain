#!/usr/bin/env bash

set -euo pipefail

cache_protocol="${CACHE_PROTOCOL_OVERRIDE:-http}"
cache_known_domain_suffix="${DOMAIN_SUFFIX_OVERRIDE:-.events.infra.holochain.org}"
diff_path=${TMPDIR:-/tmp/}holochain-local-cache.diff

function print_usage {
  echo "Usage: $0 use <event-key>"
  echo "Usage: $0 cleanup"
  exit 1
}

function prompt_and_apply_patch {
  local config_path="$1"

  printf "\nPrepared Nix config change:\n\n"
  cat "$diff_path"

  read -p "Apply this change? (y/n) " -n 1 -r
  echo
  if [[ $REPLY =~ ^[Yy]$ ]]; then
    if test -w "$config_path"; then
      < "$diff_path" patch -b -d/ -p0 -u
    else
      echo "This file is not writable by the current user, trying as root"
      # shellcheck disable=SC2002
      cat "$diff_path" | sudo patch -b -d/ -p0 -u
    fi
  fi
}

function update_config {
  local key="$1"
  local value_to_add="$2"
  local config_path="$3"

  # Key exists, update its value
  if grep -q "$key" "$config_path"; then
    # Value also found, nothing left to do
    if grep -q "$value_to_add" "$config_path"; then
      echo "The config for [$key] in [$config_path] already contains [$value_to_add], nothing to do"
    else
      # Otherwise, update the value and show a diff to apply

      set +e
      rm -f "$diff_path"
      # shellcheck disable=SC2094
      < "$config_path" sed -e "s/$key =/$key = ${value_to_add//\//\\/}/" | diff -u "$config_path" - > "$diff_path"
      has_diff=$?
      set -e

      if test $has_diff -eq 1; then
        prompt_and_apply_patch "$config_path"
      else
        echo "Nothing to change in [$config_path}]"
      fi
    fi
  else
    # The key does not exist, so create a new entry with this single value

    echo "Adding new config for [$key] with value [$value_to_add] to [$config_path]"

    if test -w "$config_path"; then
      echo "$key = $value_to_add" >> "$config_path"
    else
      echo "This file is not writable by the current user, trying as root"
      echo "$key = $value_to_add" | sudo tee -a "$config_path"
    fi
  fi
}

if test $# -eq 2; then
  cache_url="${cache_protocol}://$2$cache_known_domain_suffix"
  command=$1

  if test "$command" = "use"; then
    echo "Will configure Nix to use the local cache [${cache_url}]"
  else
    print_usage
  fi
elif test $# -eq 1; then
  cache_url="$cache_known_domain_suffix"
  command=$1

  if test "$command" = "cleanup"; then
    echo "Will remove the local cache [${cache_url}] and trusted public key from Nix configuration"
  else
    print_usage
  fi
else
  print_usage
fi

echo "Attempting to detect your Nix install type..."

nix_profile_path="/nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh"
single_user_profile_path="$HOME/.nix-profile/etc/profile.d/nix.sh"

# Adapted from https://github.com/lilyball/nix-env.fish/blob/00c6cc762427efe08ac0bd0d1b1d12048d3ca727/conf.d/nix-env.fish
if test -f $nix_profile_path; then
  # The path exists. Double-check that this is a multi-user install.
  # We can't just check for ~/.nix-profile/… because this may be a single-user install running as
  # the wrong user.

  store_info=$(ls -nd /nix/store 2>/dev/null)
  read -ra store_info_parts <<< "$store_info"
  owner="${store_info_parts[2]}"

  if ! test -k "/nix/store" || ! test "$owner" -eq 0; then
    # /nix/store is either not owned by root or not sticky. Assume single-user.
    nix_profile_path=$single_user_profile_path
  fi
else
  # The path doesn't exist. Assume single-user
  nix_profile_path=$single_user_profile_path
fi

nix_config="/etc/nix/nix.conf"

# Select a config file to edit https://nixos.org/manual/nix/unstable/command-ref/conf-file.html
if test "$nix_profile_path" = "$single_user_profile_path"; then
  echo "Detected a single-user install"
  # Note that NIX_USER_CONF_FILES and XDG_CONFIG_DIRS are not being supported here.
  nix_config="${XDG_CONFIG_HOME:-$HOME/.config/}nix/nix.conf"
else
  echo "Detected a multi-user install"
  if test ${NIX_CONF_DIR+x} ; then
    nix_config="{$NIX_CONF_DIR}nix.conf"
  fi
fi

if ! test -f "$nix_config"; then
  echo "Expected config file [$nix_config}] does not exist"
  exit 1
fi

echo "Using $nix_config"

case $command in
  "use")
    update_config "extra-substituters" "$cache_url" "$nix_config"
    update_config "extra-trusted-public-keys" "${PUBLIC_KEY_OVERRIDE:-$2$cache_known_domain_suffix:5UYNvUeMRb15qTR/u5nPBo13xjE0H3HXEtjAFDUrYvI=}" "$nix_config"
  ;;
  "cleanup")
    cleanup_paths=(
      "$nix_config" # Selected config location
      "${NIX_CONF_DIR:-/etc/nix/}nix.conf" # Best guess multi-user config location
      "${XDG_CONFIG_HOME:-$HOME/.config/}nix/nix.conf" # best guess single-user config location
    )

    for config_path in "${cleanup_paths[@]}"; do
      if test -f "$config_path"; then
        set +e
        rm -f "$diff_path"
        # shellcheck disable=SC2094
        < "$config_path" sed -e "s/http:\/\/[[:alnum:]]*${cache_known_domain_suffix} *//g" \
          | sed -e "s/[[:alnum:]]*${cache_known_domain_suffix}:[[:alnum:]/=]* *//g" \
          | sed -e '/extra-substituters =[[:blank:]]*$/d' \
          | sed -e '/extra-trusted-public-keys =[[:blank:]]*$/d' \
          | diff -u --ignore-blank-lines "$config_path" - > "$diff_path"
        has_diff=$?
        set -e

        if test $has_diff -eq 1; then
          prompt_and_apply_patch "$config_path"
        else
          echo "Nothing to change in [$config_path]"
        fi
      else
        echo "Skipping config file [$config_path] as it does not exist"
      fi
    done
  ;;
  *)
    # Should not reach here, just being safe
    print_usage
esac

if command -v systemctl &> /dev/null; then
  echo "Restarting the Nix daemon with systemctl..."
  sudo systemctl restart nix-daemon
elif command -v launchctl &> /dev/null; then
  echo "Restarting the Nix daemon with launchctl..."
  sudo launchctl list
  sudo launchctl stop system/org.nixos.nix-daemon
  echo "Stopped"
  sudo launchctl start system/org.nixos.nix-daemon
  echo "Started"
else
  echo "Unable to restart the Nix daemon, please restart it manually"
fi

echo "All done!"
