#!/usr/bin/env bash
set -euo pipefail

# The sparse index can lag a successful upload by longer than a fixed sleep.
# A retry always publishes from the release tag, never newer main source.
for crate in weave-core weave-crdt weave-certify weave-cli weave-driver weave-mcp; do
  version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "crates/$crate/Cargo.toml" | head -1)
  test -n "$version"
  if ! cargo info --registry crates-io "$crate@$version" >/dev/null 2>&1; then
    log=$(mktemp)
    if cargo publish -p "$crate" --no-verify >"$log" 2>&1; then
      cat "$log"
    else
      status=$?
      cat "$log"
      if ! grep -qi 'already exists\|already uploaded' "$log"; then
        rm -f "$log"
        exit "$status"
      fi
    fi
    rm -f "$log"
  fi
  available=false
  for attempt in $(seq 1 60); do
    if cargo info --registry crates-io "$crate@$version" >/dev/null 2>&1; then
      available=true
      break
    fi
    echo "Waiting for $crate@$version in crates.io ($attempt/60)"
    sleep 15
  done
  if [ "$available" != true ]; then
    echo "::error::$crate@$version is not available in crates.io after 15 minutes"
    exit 1
  fi
  echo "$crate@$version is available"
done
