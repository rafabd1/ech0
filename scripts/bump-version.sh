#!/usr/bin/env bash
# ech0 — Version bump script
# Usage: ./scripts/bump-version.sh <version>
# Example: ./scripts/bump-version.sh 1.2.0
#
# Updates the version field in:
#   - package.json
#   - src-tauri/tauri.conf.json
#   - src-tauri/Cargo.toml
#
# After running, commit the changes and tag the release:
#   git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml
#   git commit -m "chore: bump version to $VERSION"
#   git tag "v$VERSION"
#   git push && git push --tags

set -euo pipefail

VERSION="${1:-}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [[ -z "$VERSION" ]]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 1.2.0"
    exit 1
fi

# Validate semver (loose: digits and dots)
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][a-zA-Z0-9.]+)?$ ]]; then
    echo "ERROR: '$VERSION' does not look like a valid version (expected semver, e.g. 1.2.0)"
    exit 1
fi

echo "==> Bumping version to $VERSION"

# ── package.json ──────────────────────────────────────────────────────────────
cd "$ROOT"
npm pkg set version="$VERSION"
echo "    package.json          ✓"

# ── tauri.conf.json ───────────────────────────────────────────────────────────
node -e "
  const fs = require('fs');
  const p = 'src-tauri/tauri.conf.json';
  const cfg = JSON.parse(fs.readFileSync(p, 'utf8'));
  cfg.version = '$VERSION';
  fs.writeFileSync(p, JSON.stringify(cfg, null, 2) + '\n');
"
echo "    src-tauri/tauri.conf.json  ✓"

# ── Cargo.toml ────────────────────────────────────────────────────────────────
# Only update the [package] section's version line (not dependency versions).
# Works on GNU sed (Linux) and BSD sed (macOS).
sed -i.bak '/^\[package\]/,/^\[/{s/^version = ".*"/version = "'"$VERSION"'"/}' \
    src-tauri/Cargo.toml
rm -f src-tauri/Cargo.toml.bak
echo "    src-tauri/Cargo.toml  ✓"

echo ""
echo "==> Done. Next steps:"
echo "    git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml"
echo "    git commit -m \"chore: bump version to $VERSION\""
echo "    git tag v$VERSION"
echo "    git push && git push --tags"
