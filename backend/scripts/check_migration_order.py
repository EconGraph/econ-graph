#!/usr/bin/env python3
"""
Database Migration Order Validation Script

This script validates that database migrations are in chronological order
and that new migrations don't have dates that precede existing migrations.

Usage:
    python3 scripts/check_migration_order.py

Exit codes:
    0: All migrations are in correct order
    1: Migration order issues found
"""

import os
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import List, Tuple, Optional


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

    return None


def get_migration_files(migrations_dir: str) -> List[Tuple[datetime, str, str]]:
    """
    Get all migration files sorted by timestamp.

    Returns: List of (timestamp, name, full_path) tuples
    """
    migrations_path = Path(migrations_dir)
    if not migrations_path.exists():
        print(f"❌ Migrations directory not found: {migrations_dir}")
        return []

    migrations = []

    for item in migrations_path.iterdir():
        if item.is_dir() and not item.name.startswith('.'):
            parsed = parse_migration_filename(item.name)
            if parsed:
                timestamp, name = parsed
                migrations.append((timestamp, name, str(item)))
            else:
                print(f"⚠️  Skipping invalid migration directory: {item.name}")

    return sorted(migrations, key=lambda x: x[0])


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

        if curr_timestamp <= prev_timestamp:
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
        timestamp_str = timestamp.strftime('%Y-%m-%d %H:%M:%S')
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


def main():
    """Main function to run migration validation."""
    print("🚀 Starting database migration order validation...")

    # Get migrations directory
    script_dir = Path(__file__).parent
    backend_dir = script_dir.parent
    migrations_dir = backend_dir / "migrations"

    # Get all migration files
    migrations = get_migration_files(str(migrations_dir))

    if not migrations:
        print("❌ No valid migrations found")
        return 1

    print(f"📁 Found {len(migrations)} migrations:")
    for timestamp, name, path in migrations:
        print(f"   {timestamp.strftime('%Y-%m-%d %H:%M:%S')} - {name}")

    print()

    # Run all checks
    all_checks_passed = True

    # Check migration order
    if not check_migration_order(migrations):
        all_checks_passed = False

    print()

    # Check for duplicate timestamps
    if not check_for_duplicate_timestamps(migrations):
        all_checks_passed = False

    print()

    # Check naming conventions
    if not check_migration_naming_conventions(migrations):
        all_checks_passed = False

    print()

    if all_checks_passed:
        print("✅ All migration validation checks passed!")
        return 0
    else:
        print("❌ Migration validation failed!")
        print("   Please fix the issues above before committing.")
        return 1


if __name__ == "__main__":
    sys.exit(main())
