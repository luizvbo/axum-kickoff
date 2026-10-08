#!/bin/sh
# Runs a cargo check that needs a parseable Cargo.toml.
#
# In the template checkout the manifest contains Liquid placeholders, so the
# tool is run inside a freshly rendered default project in a temp dir — the
# same thing .github/workflows/template-ci.yml's deps job does. This keeps
# prek hooks honest in the template repo instead of skipping them.
#
# In a generated project cargo-generate.toml is absent and the tool runs
# directly (fmt stays a fixer there).
#
# Usage: rendered-check.sh <fmt|deny|machete>
set -eu

check="${1:?usage: rendered-check.sh <fmt|deny|machete>}"

if [ -f cargo-generate.toml ]; then
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    cargo generate --path . --name rendered-check \
        --destination "$tmp" --vcs none --silent
    cd "$tmp/rendered-check"
    case "$check" in
        fmt) exec cargo fmt --check ;;
        deny) exec cargo deny check ;;
        machete) exec cargo machete ;;
    esac
else
    case "$check" in
        fmt) exec cargo fmt ;;
        deny) exec cargo deny check ;;
        machete) exec cargo machete ;;
    esac
fi
echo "rendered-check.sh: unknown check '$check'" >&2
exit 2
