#!/usr/bin/env python3
"""Validate local compatibility facades before auditing their real registry implementations."""
import json
from pathlib import Path
import sys
import tomllib

ADAPTERS = {
    "zstd": {
        "legacy": "0.13.3", "patched": "0.14.3", "alias": "zstd_patched",
        "features": {"arrays", "bindgen", "debug", "doc-cfg", "experimental", "fat-lto", "legacy",
                     "no_asm", "pkg-config", "thin", "thin-lto", "wasm", "zdict_builder", "zstdmt"},
        "default": ["legacy", "arrays", "zdict_builder"],
    },
    "quick-xml": {
        "legacy": "0.39.4", "patched": "0.42.0", "alias": "quick_xml_patched",
        "features": {"async-tokio", "encoding", "escape-html", "overlapped-lists", "serde-types", "serialize"},
        "default": [],
    },
}
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
FIXED_ADVISORIES = {"RUSTSEC-2026-0194", "RUSTSEC-2026-0195", "RUSTSEC-2026-0391"} | {
    f"RUSTSEC-2026-{number:04}" for number in range(395, 407)
}


def validate(backend):
    problems = []
    workspace = tomllib.loads((backend / "Cargo.toml").read_text())
    packages = tomllib.loads((backend / "Cargo.lock").read_text())["package"]
    evidence = []
    if not {"compat/zstd", "compat/quick-xml"} <= set(workspace["workspace"].get("exclude", [])):
        problems.append("Facades must stay dependency-only so workspace all-features does not activate unused upstream features")
    for name, expected in ADAPTERS.items():
        adapter = backend / "compat" / name
        files = {str(path.relative_to(adapter)) for path in adapter.rglob("*") if path.is_file()}
        if files != {"Cargo.toml", "src/lib.rs"} or any(path.is_symlink() for path in adapter.rglob("*")):
            problems.append(f"{name}: facade must contain only its manifest and one non-symlink re-export source")
        manifest = tomllib.loads((adapter / "Cargo.toml").read_text())
        if set(manifest) != {"package", "dependencies", "features"}:
            problems.append(f"{name}: unexpected manifest sections could add implementation/build code")
        package = manifest["package"]
        if set(package) != {"name", "version", "edition", "license", "publish", "description"} or (
            package["name"], package["version"], package["edition"], package["license"], package["publish"]
        ) != (name, expected["legacy"], "2024", "MIT", False):
            problems.append(f"{name}: compatibility identity or source-only package policy changed")
        if manifest["dependencies"] != {expected["alias"]: {
            "package": name, "version": "=" + expected["patched"], "default-features": False
        }}:
            problems.append(f"{name}: facade must depend only on its exact patched registry implementation")
        features = {feature: [expected["alias"] + "/" + feature] for feature in expected["features"]}
        features["default"] = expected["default"]
        if manifest["features"] != features:
            problems.append(f"{name}: every legacy feature must forward directly to the patched implementation")
        source = "\n".join(line for line in (adapter / "src/lib.rs").read_text().splitlines()
                           if line.strip() and not line.strip().startswith("//!"))
        if source != f'#![forbid(unsafe_code)]\npub use {expected["alias"]}::*;':
            problems.append(f"{name}: source must contain only unsafe prohibition and direct re-export")
        if workspace["patch"]["crates-io"].get(name) != {"path": "compat/" + name}:
            problems.append(f"{name}: workspace patch must select the inspected compatibility facade")
        resolved = [package for package in packages if package["name"] == name]
        facade = next((package for package in resolved if package["version"] == expected["legacy"] and "source" not in package), None)
        upstream = next((package for package in resolved if package["version"] == expected["patched"] and package.get("source") == REGISTRY), None)
        if len(resolved) != 2 or facade is None or upstream is None or not upstream.get("checksum"):
            problems.append(f"{name}: lock must contain only the facade plus checksummed patched registry implementation")
        elif facade.get("dependencies") != [name + " " + expected["patched"]]:
            problems.append(f"{name}: resolved facade dependency is not the inspected patched implementation")
        evidence.append({"facade": name + " " + expected["legacy"], "implementation": name + " " + expected["patched"], "registry_audited": upstream is not None})
    for package in packages:
        if package["name"] == "zstd-safe" and package["version"] != "8.2.0":
            problems.append("A zstd-safe version outside the inspected patched dependency graph returned")
    for policy in (backend / ".cargo/audit.toml", backend / "deny.toml"):
        text = policy.read_text()
        ignores = tomllib.loads(text).get("advisories", {}).get("ignore", [])
        ids = {item if isinstance(item, str) else item["id"] for item in ignores}
        if ids & FIXED_ADVISORIES:
            problems.append(f"{policy.name}: fixed XML/zstd advisories must not be waived")
    return problems, evidence


def main():
    try:
        problems, evidence = validate(Path(__file__).resolve().parents[1])
    except (OSError, KeyError, ValueError) as error:
        print(f"Cannot validate security adapter inventory: {error}", file=sys.stderr)
        return 1
    if problems:
        for problem in problems:
            print(problem, file=sys.stderr)
        return 1
    print(json.dumps({"status": "passed", "implementations": evidence}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
