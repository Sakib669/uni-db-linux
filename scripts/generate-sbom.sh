#!/usr/bin/env bash
# Generates a CycloneDX SBOM for the workspace into target/sbom.cdx.json.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
python3 scripts/generate-sbom.py
