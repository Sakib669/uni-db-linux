#!/usr/bin/env python3
"""Writes a CycloneDX 1.5 JSON SBOM from cargo metadata.

Works offline, so release jobs and contributors produce the same document.
"""

import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def cargo_metadata():
    output = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--all-features"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(output.stdout)


def component(package):
    name = package["name"]
    version = package["version"]
    component = {
        "type": "library",
        "name": name,
        "version": version,
        "purl": f"pkg:cargo/{name}@{version}",
        "bom-ref": f"pkg:cargo/{name}@{version}",
    }
    if package.get("license"):
        component["licenses"] = [{"license": {"id": package["license"]}}]
    return component


def main():
    metadata = cargo_metadata()
    packages = sorted(metadata["packages"], key=lambda item: (item["name"], item["version"]))
    components = [component(package) for package in packages]
    workspace = metadata["workspace_members"]
    workspace_name = next(
        package["name"] for package in packages if package["id"] in workspace
    )
    document = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "name": workspace_name,
                "version": packages[0]["version"],
            }
        },
        "components": components,
    }
    payload = json.dumps(document, indent=2, sort_keys=True)
    output = ROOT / "target" / "sbom.cdx.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(payload + "\n", encoding="utf-8")
    digest = hashlib.sha256(payload.encode("utf-8")).hexdigest()
    print(f"SBOM written: {output}")
    print(f"components: {len(components)}")
    print(f"sha256: {digest}")


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        print(f"FAIL: cargo metadata failed: {error}", file=sys.stderr)
        sys.exit(1)
