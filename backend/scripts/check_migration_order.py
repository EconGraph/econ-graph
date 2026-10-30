#!/usr/bin/env python3
"""
Database Migration Order Validation Script

This script validates that database migrations are in chronological order
and that new migrations don't have dates that precede existing migrations.

backend/migrations holds two migration sets, app/ and data/ (see
docs/development/MIGRATIONS.md). Each set is checked on its own, and then across
both: when one database holds both sets they share one __diesel_schema_migrations
table, so a version may appear in only one set. The exception is
00000000000000_diesel_initial_setup, which both sets carry and which must be
identical in both.

Usage:
    python3 scripts/check_migration_order.py

Exit codes:
    0: All migrations are in correct order
    1: Migration order issues found
"""

import argparse
import json
import subprocess
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Tuple, Optional


def parse_migration_filename(filename: str) -> Optional[Tuple[datetime, str]]:
    """
    Parse migration filename to extract timestamp and name.

    Supports multiple formats:
    - YYYY-MM-DD-HHMMSS_migration_name
    - YYYY-MM-DD-HHMMSS-0000_migration_name
    - 00000000000000_migration_name (diesel initial setup)

    Returns: (datetime, migration_name) or None if invalid format
    """
    # Pattern 1: YYYY-MM-DD-HHMMSS_migration_name
    pattern1 = r'^(\d{4}-\d{2}-\d{2}-\d{6})_(.+)$'
    match1 = re.match(pattern1, filename)

    if match1:
        timestamp_str, name = match1.groups()
        try:
            timestamp = datetime.strptime(timestamp_str, '%Y-%m-%d-%H%M%S')
            return timestamp, name
        except ValueError:
            pass

    # Pattern 2: YYYY-MM-DD-HHMMSS-0000_migration_name
    pattern2 = r'^(\d{4}-\d{2}-\d{2}-\d{6})-(\d{4})_(.+)$'
    match2 = re.match(pattern2, filename)

    if match2:
        timestamp_str, sequence, name = match2.groups()
        try:
            timestamp = datetime.strptime(timestamp_str, '%Y-%m-%d-%H%M%S')
            return timestamp, name
        except ValueError:
            pass

    # Pattern 3: 00000000000000_migration_name (diesel initial setup)
    pattern3 = r'^00000000000000_(.+)$'
    match3 = re.match(pattern3, filename)

    if match3:
        name = match3.group(1)
        # Use a very early timestamp for diesel initial setup
        timestamp = datetime(1970, 1, 1, 0, 0, 0)
        return timestamp, name

    # Pattern 4: YYYYMMDDHHMMSS_migration_name (no dashes; Diesel reads the same version)
    pattern4 = r'^(\d{14})_(.+)$'
    match4 = re.match(pattern4, filename)

    if match4:
        timestamp_str, name = match4.groups()
        try:
            timestamp = datetime.strptime(timestamp_str, '%Y%m%d%H%M%S')
            return timestamp, name
        except ValueError:
            pass

    return None


def get_migration_files(migrations_dir: str) -> Tuple[List[Tuple[datetime, str, str]], bool]:
    """
    Get all migration files sorted by their exact Diesel version.

    Every directory counts: Diesel runs any directory with an up.sql, whatever its name, so a
    directory this script cannot parse, or one without an up.sql, is an error rather than
    something to skip (a skipped one would escape the duplicate-version checks).

    Returns: (list of (timestamp, name, full_path) tuples, whether every directory parsed)
    """
    migrations_path = Path(migrations_dir)
    if not migrations_path.exists():
        print(f"❌ Migrations directory not found: {migrations_dir}")
        return [], False

    migrations = []
    ok = True

    for item in sorted(migrations_path.iterdir()):
        if item.is_dir() and not item.name.startswith('.'):
            parsed = parse_migration_filename(item.name)
            if not (item / "up.sql").is_file():
                print(f"❌ Migration directory without an up.sql: {item}")
                ok = False
            elif parsed is None or not re.fullmatch(r'\d{14,}', diesel_version(item.name)):
                print(f"❌ Cannot read a migration version from: {item}")
                print("   Name it YYYY-MM-DD-HHMMSS_name (or YYYYMMDDHHMMSS_name).")
                ok = False
            else:
                timestamp, name = parsed
                migrations.append((timestamp, name, str(item)))

    return sorted(migrations, key=lambda x: diesel_version(Path(x[2]).name)), ok


def check_migration_order(migrations: List[Tuple[datetime, str, str]]) -> bool:
    """
    Check if migrations are in correct chronological order.

    Returns: True if order is correct, False otherwise
    """
    if len(migrations) <= 1:
        return True

    print("🔍 Checking migration order...")

    issues_found = False

    for i in range(1, len(migrations)):
        prev_timestamp, prev_name, prev_path = migrations[i-1]
        curr_timestamp, curr_name, curr_path = migrations[i]

        if diesel_version(Path(curr_path).name) <= diesel_version(Path(prev_path).name):
            print(f"❌ Migration order issue found:")
            print(f"   Previous: {prev_timestamp.strftime('%Y-%m-%d %H:%M:%S')} - {prev_name}")
            print(f"   Current:  {curr_timestamp.strftime('%Y-%m-%d %H:%M:%S')} - {curr_name}")
            print(f"   Current migration timestamp is not after previous migration")
            issues_found = True

    return not issues_found


def check_for_duplicate_timestamps(migrations: List[Tuple[datetime, str, str]]) -> bool:
    """
    Check for duplicate timestamps in migrations.

    Returns: True if no duplicates, False otherwise
    """
    print("🔍 Checking for duplicate timestamps...")

    timestamp_counts = {}
    for timestamp, name, path in migrations:
        timestamp_str = diesel_version(Path(path).name)
        if timestamp_str not in timestamp_counts:
            timestamp_counts[timestamp_str] = []
        timestamp_counts[timestamp_str].append((name, path))

    issues_found = False
    for timestamp_str, migrations_list in timestamp_counts.items():
        if len(migrations_list) > 1:
            print(f"❌ Duplicate timestamp found: {timestamp_str}")
            for name, path in migrations_list:
                print(f"   - {name} ({path})")
            issues_found = True

    return not issues_found


def check_migration_naming_conventions(migrations: List[Tuple[datetime, str, str]]) -> bool:
    """
    Check migration naming conventions.

    Returns: True if naming is correct, False otherwise
    """
    print("🔍 Checking migration naming conventions...")

    issues_found = False

    for timestamp, name, path in migrations:
        # Check for reasonable naming patterns
        if not re.match(r'^[a-z][a-z0-9_]*[a-z0-9]$', name):
            print(f"⚠️  Migration name doesn't follow convention: {name}")
            print(f"   Expected: lowercase with underscores, no leading/trailing underscores")
            print(f"   Path: {path}")
            # Don't fail on this, just warn

        # Check for very short names
        if len(name) < 3:
            print(f"❌ Migration name too short: {name}")
            print(f"   Path: {path}")
            issues_found = True

    return not issues_found


MIGRATION_SETS = ("data", "app")
SHARED_VERSION = "00000000000000"


def diesel_version(dirname: str) -> str:
    """The version Diesel records for a migration directory: its name up to the first "_",
    without dashes (migrations_internals::version_from_string)."""
    return dirname.split("_", 1)[0].replace("-", "")


def check_migrations_root(migrations_root: Path) -> bool:
    """
    Check that the migrations root holds only the migration sets, and nothing else that
    Diesel or a reader could take for a migration.

    Returns: True if the layout is correct, False otherwise
    """
    print("🔍 Checking the migrations directory layout...")
    ok = True
    for item in sorted(migrations_root.iterdir()):
        if item.name.startswith("."):
            continue
        if not (item.is_dir() and item.name in MIGRATION_SETS):
            print(f"❌ Unexpected entry in {migrations_root}: {item.name}")
            print(f"   Migrations go in one of: {', '.join(s + '/' for s in MIGRATION_SETS)}")
            ok = False
    for name in MIGRATION_SETS:
        if not (migrations_root / name).is_dir():
            print(f"❌ Missing migration set: {migrations_root / name}")
            ok = False
    return ok


def check_versions_across_sets(sets: Dict[str, List[Tuple[datetime, str, str]]]) -> bool:
    """
    Check that no version appears in more than one set, except the shared Diesel setup
    migration, whose copies must be identical.

    Returns: True if the sets are disjoint, False otherwise
    """
    print("🔍 Checking for versions repeated across migration sets...")
    owners: Dict[str, List[str]] = {}
    for migrations in sets.values():
        for _, _, path in migrations:
            owners.setdefault(diesel_version(Path(path).name), []).append(path)

    ok = True
    for version, paths in sorted(owners.items()):
        if len(paths) < 2:
            continue
        if version != SHARED_VERSION:
            print(f"❌ Version {version} is used in more than one migration set:")
            for path in paths:
                print(f"   - {path}")
            print("   A database holding both sets shares one __diesel_schema_migrations table, so")
            print("   one of these would silently never run there. Give it a new version.")
            ok = False
            continue
        first = Path(paths[0])
        for other in map(Path, paths[1:]):
            for sql in ("up.sql", "down.sql"):
                a, b = first / sql, other / sql
                if not (a.is_file() and b.is_file() and a.read_bytes() == b.read_bytes()):
                    print(f"❌ {a} and {b} differ; the shared setup migration must be identical in every set")
                    ok = False
    return ok


# The setup and split baselines cannot participate in release squashing.
PERMANENT_VERSIONS = {SHARED_VERSION, "20261021000100", "20261021000200"}
FINAL_RELEASE = re.compile(r"v\d+\.\d+\.\d+$")
SEED_NAME = re.compile(r"\d[\d-]*_seed_([a-z0-9_]+)_reference_codes$")


def git(repo: Path, *args: str) -> str:
    """Run read-only Git commands; missing refs/history are validation failures."""
    result = subprocess.run(["git", "-C", str(repo), *args], text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
    return result.stdout


def migration_tree(repo: Path, ref: str) -> Dict[str, Dict[str, str]]:
    """Migration directories and all their tracked blob identities at an exact commit."""
    tree: Dict[str, Dict[str, str]] = {}
    for record in git(repo, "ls-tree", "-rz", ref, "--", "backend/migrations").split("\0"):
        if not record:
            continue
        metadata, path = record.split("\t", 1)
        parts = Path(path).parts
        if len(parts) >= 5 and parts[2] in MIGRATION_SETS and not parts[3].startswith("."):
            directory = "/".join(parts[:4])
            tree.setdefault(directory, {})["/".join(parts[4:])] = metadata
    return tree


def working_migration_tree(repo: Path) -> Dict[str, Dict[str, str]]:
    tree: Dict[str, Dict[str, str]] = {}
    for plane in MIGRATION_SETS:
        root = repo / "backend/migrations" / plane
        if not root.is_dir():
            continue
        for directory in root.iterdir():
            if not directory.is_dir() or directory.name.startswith("."):
                continue
            files = {}
            for path in directory.rglob("*"):
                if path.is_file():
                    # hash-object reads without writing the object database. Include mode so
                    # replacing SQL with a symlink cannot evade immutability checks.
                    mode = "120000" if path.is_symlink() else ("100755" if path.stat().st_mode & 0o111 else "100644")
                    oid = git(repo, "hash-object", "--", str(path)).strip()
                    files[str(path.relative_to(directory))] = f"{mode} blob {oid}"
            tree[str(directory.relative_to(repo))] = files
    return tree


def published_migrations(repo: Path) -> set:
    """Final product tags protect their versions; RC and train tags never enable squashing."""
    published = set()
    for tag in git(repo, "tag", "--list", "v*").splitlines():
        if FINAL_RELEASE.fullmatch(tag):
            for directory in migration_tree(repo, f"refs/tags/{tag}"):
                published.add(diesel_version(Path(directory).name))
            # Older final releases may predate the app/data split.
            for name in git(repo, "ls-tree", "--name-only", f"refs/tags/{tag}:backend/migrations").splitlines():
                if parse_migration_filename(name):
                    published.add(diesel_version(name))
    return published


def release_squash_paths(target: Dict, current: Dict, published: set) -> set:
    """Recognize only a complete unshipped app suffix folded into its final version.

    SQL equivalence and the required fresh/upgrade guards are still reviewed and checked
    with compare_migrations.sh; this exception only controls migration-history eligibility.
    """
    app = sorted((p for p in target if Path(p).parts[2] == "app"),
                 key=lambda p: diesel_version(Path(p).name))
    changed = [p for p in app if current.get(p) != target[p]]
    if len(changed) < 2:
        return set()
    first = app.index(changed[0])
    suffix = app[first:]
    if changed != suffix:
        return set()
    versions = [diesel_version(Path(p).name) for p in suffix]
    newest_shipped = max((diesel_version(Path(p).name) for p in app
                          if diesel_version(Path(p).name) in published), default="")
    if any(v in published or v in PERMANENT_VERSIONS or v <= newest_shipped for v in versions):
        return set()
    replacements = [p for p in current if Path(p).parts[2] == "app"
                    and diesel_version(Path(p).name) in versions]
    if len(replacements) != 1 or diesel_version(Path(replacements[0]).name) != versions[-1]:
        return set()
    return set(suffix + replacements)


def check_against_target(repo: Path, base_ref: str) -> bool:
    """Compare against target *tip*, not merge-base: concurrent additions are already run."""
    base = git(repo, "rev-parse", "--verify", "--end-of-options", f"{base_ref}^{{commit}}").strip()
    target = migration_tree(repo, base)
    current = working_migration_tree(repo)
    if not target:
        raise ValueError(f"Merge target {base_ref} has no migration sets; refusing unchecked history")
    published = published_migrations(repo)
    squash = release_squash_paths(target, current, published)
    high_water = max(diesel_version(Path(p).name) for p in target)
    ok = True
    if squash:
        print("⚠️  Eligible app release squash: also run compare_migrations.sh against the pre-squash commit")
    for directory, files in target.items():
        if current.get(directory) == files or directory in squash:
            continue
        # Recording reference seeds replaces, never edits, a source's previous version.
        match = SEED_NAME.fullmatch(Path(directory).name)
        seed_replacement = (Path(directory).parts[2] == "data" and directory not in current and match
                            and any(Path(p).parts[2] == "data" and p not in target
                                    and (m := SEED_NAME.fullmatch(Path(p).name))
                                    and m.group(1) == match.group(1)
                                    and diesel_version(Path(p).name) > high_water for p in current))
        if seed_replacement:
            continue
        print(f"❌ Protected merged migration was edited or removed: {directory}")
        print("   Add a new forward migration; only eligible app release squashes and newer same-source seed replacements are exempt.")
        ok = False
    for directory in current.keys() - target.keys() - squash:
        version = diesel_version(Path(directory).name)
        if version <= high_water:
            print(f"❌ New migration {directory} does not sort after target {base_ref} ({high_water})")
            print("   Rebase onto the target and renumber only your unmerged migration after both sets.")
            ok = False
    return ok


def event_base_ref(event_path: Optional[str], event_name: Optional[str]) -> Optional[str]:
    """Use event target SHA (including merge-queue base), never the PR's fork-point."""
    if not event_path:
        return None
    event = json.loads(Path(event_path).read_text())
    if event_name == "pull_request":
        return event["pull_request"]["base"]["sha"]
    if event_name == "merge_group":
        return event["merge_group"]["base_sha"]
    if event_name == "push" and event.get("before") and set(event["before"]) != {"0"}:
        return event["before"]
    return None


def main():
    """Main function to run migration validation."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-ref", help="Exact merge-target commit/ref; local default: origin/main")
    parser.add_argument("--event-path", help="GitHub event JSON; target is selected by event type")
    parser.add_argument("--event-name", help="GitHub event name")
    args = parser.parse_args()
    print("🚀 Starting database migration order validation...")

    script_dir = Path(__file__).parent
    backend_dir = script_dir.parent
    migrations_root = backend_dir / "migrations"

    all_checks_passed = check_migrations_root(migrations_root)
    print()

    sets: Dict[str, List[Tuple[datetime, str, str]]] = {}
    for set_name in MIGRATION_SETS:
        migrations_dir = migrations_root / set_name
        if not migrations_dir.is_dir():
            continue
        print(f"📦 Migration set {set_name}/")
        migrations, parsed_all = get_migration_files(str(migrations_dir))
        if not parsed_all:
            all_checks_passed = False
        if not migrations:
            print(f"❌ No valid migrations found in {migrations_dir}")
            all_checks_passed = False
            continue
        sets[set_name] = migrations

        print(f"📁 Found {len(migrations)} migrations:")
        for timestamp, name, path in migrations:
            print(f"   {timestamp.strftime('%Y-%m-%d %H:%M:%S')} - {name}")
        print()

        if not check_migration_order(migrations):
            all_checks_passed = False
        print()
        if not check_for_duplicate_timestamps(migrations):
            all_checks_passed = False
        print()
        if not check_migration_naming_conventions(migrations):
            all_checks_passed = False
        print()

    if not check_versions_across_sets(sets):
        all_checks_passed = False
    print()

    try:
        base_ref = args.base_ref or event_base_ref(args.event_path, args.event_name) or "origin/main"
        repo = backend_dir.parent.resolve()
        if not check_against_target(repo, base_ref):
            all_checks_passed = False
    except (subprocess.CalledProcessError, ValueError, KeyError, OSError) as error:
        print(f"❌ Cannot validate target migration history: {error}")
        all_checks_passed = False

    if all_checks_passed:
        print("✅ All migration validation checks passed!")
        return 0
    else:
        print("❌ Migration validation failed!")
        print("   Please fix the issues above before committing.")
        return 1


if __name__ == "__main__":
    sys.exit(main())
