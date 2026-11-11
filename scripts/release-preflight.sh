#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

allow_dirty=0
expected_version=""
while (($#)); do
  case "$1" in
    --allow-dirty) allow_dirty=1 ;;
    --version)
      (($# >= 2)) || { echo "--version requires a value" >&2; exit 64; }
      shift
      expected_version="$1"
      ;;
    *) echo "usage: $0 [--allow-dirty] [--version X.Y.Z]" >&2; exit 64 ;;
  esac
  shift
done

for command in cargo cargo-deny git node python3 shellcheck; do
  command -v "$command" >/dev/null 2>&1 || { echo "missing required command: $command" >&2; exit 69; }
done

package_version="$(cargo metadata --locked --no-deps --format-version 1 \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
[[ "$package_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]] \
  || { echo "Cargo.toml contains invalid SemVer: $package_version" >&2; exit 65; }
if [[ -n "$expected_version" && "$expected_version" != "$package_version" ]]; then
  echo "version mismatch: requested $expected_version, Cargo.toml has $package_version" >&2
  exit 65
fi
grep -q "^## \[$package_version\]" CHANGELOG.md \
  || { echo "CHANGELOG.md has no section for $package_version" >&2; exit 65; }

if (( ! allow_dirty )) && [[ -n "$(git status --porcelain)" ]]; then
  echo "working tree is not clean (use --allow-dirty only for local development)" >&2
  exit 66
fi

echo "==> release preflight for $package_version"
git diff --check
cargo fmt --all -- --check
cargo deny check advisories licenses
cargo clippy --all-targets -- -D warnings
cargo test -- --test-threads=1 --format terse
python3 -m unittest tests/test_routing_agent.py
node --check frontend/app.js
shellcheck install.sh node-bootstrap.sh transfer.sh portal-domain.sh \
  scripts/test-install.sh scripts/test-install-inner.sh scripts/release-preflight.sh \
  deploy/routing/install.sh deploy/routing/zpnet-routingctl deploy/routing/awgram-routing-remote
bash -n install.sh node-bootstrap.sh transfer.sh portal-domain.sh \
  scripts/test-install.sh scripts/test-install-inner.sh scripts/release-preflight.sh \
  deploy/routing/install.sh deploy/routing/zpnet-routingctl deploy/routing/awgram-routing-remote
cargo build --release --bins

echo "PASS: source, tests and release binaries are ready for $package_version"
echo "No tag, GitHub release, production service or database was changed."
