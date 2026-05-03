#!/usr/bin/env bash
# validate-all.sh — run every validator against every IC manifest and
# every example board. Exit non-zero if anything fails. Intended for CI.

set -euo pipefail

cd "$(dirname "$0")/.."

echo "=== Validating IC manifests ==="
for f in ic-library/*/manifest.yaml; do
    python3 tools/validate-manifest.py "$f"
done

echo
echo "=== Validating example boards ==="
for f in examples/*.yaml; do
    python3 tools/validate-board.py "$f"
done

echo
echo "All validations passed."
