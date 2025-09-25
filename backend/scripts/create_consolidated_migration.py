#!/usr/bin/env python3
"""
Create Consolidated Migration Script

This script creates a consolidated migration from the target schema dump,
extracting only the necessary DDL statements and organizing them properly.
"""

import re
import sys
from pathlib import Path


def extract_ddl_statements(schema_file: str) -> dict:
    """
    Extract DDL statements from schema dump.

    Returns: Dictionary with different types of DDL statements
    """
    with open(schema_file, 'r') as f:
        content = f.read()

    statements = {
        'extensions': [],
        'types': [],
        'tables': [],
        'indexes': [],
        'constraints': [],
        'triggers': [],
        'functions': []
    }

    # Extract extensions
    extension_pattern = r'CREATE EXTENSION IF NOT EXISTS (\w+) WITH SCHEMA public;'
    statements['extensions'] = re.findall(extension_pattern, content)

    # Extract custom types (ENUMs)
    type_pattern = r'CREATE TYPE public\.(\w+) AS ENUM \(([^)]+)\);'
    for match in re.finditer(type_pattern, content, re.DOTALL):
        type_name = match.group(1)
        enum_values = match.group(2).strip()
        statements['types'].append((type_name, enum_values))

    # Extract tables (excluding diesel migrations table)
    table_pattern = r'CREATE TABLE public\.(\w+) \(([^;]+)\);'
    for match in re.finditer(table_pattern, content, re.DOTALL):
        table_name = match.group(1)
        if table_name != '__diesel_schema_migrations':
            table_def = match.group(2).strip()
            statements['tables'].append((table_name, table_def))

    # Extract indexes
    index_pattern = r'CREATE INDEX (\w+) ON public\.(\w+) USING (\w+) \(([^)]+)\)(?: WHERE \([^)]+\))?;'
    for match in re.finditer(index_pattern, content, re.DOTALL):
        index_name = match.group(1)
        table_name = match.group(2)
        index_type = match.group(3)
        columns = match.group(4)
        where_clause = match.group(5) if match.group(5) else ''
        statements['indexes'].append((index_name, table_name, index_type, columns, where_clause))

    # Extract constraints
    constraint_pattern = r'ALTER TABLE ONLY public\.(\w+)\s+ADD CONSTRAINT (\w+) ([^;]+);'
    for match in re.finditer(constraint_pattern, content, re.DOTALL):
        table_name = match.group(1)
        constraint_name = match.group(2)
        constraint_def = match.group(3)
        statements['constraints'].append((table_name, constraint_name, constraint_def))

    # Extract triggers
    trigger_pattern = r'CREATE TRIGGER (\w+) ([^;]+);'
    for match in re.finditer(trigger_pattern, content, re.DOTALL):
        trigger_name = match.group(1)
        trigger_def = match.group(2)
        statements['triggers'].append((trigger_name, trigger_def))

    # Extract functions
    function_pattern = r'CREATE OR REPLACE FUNCTION public\.(\w+)\([^)]*\) RETURNS ([^;]+);'
    for match in re.finditer(function_pattern, content, re.DOTALL):
        func_name = match.group(1)
        func_returns = match.group(2)
        statements['functions'].append((func_name, func_returns))

    return statements


def generate_consolidated_migration(statements: dict) -> str:
    """
    Generate the consolidated migration SQL.
    """
    migration_sql = []

    # Add header
    migration_sql.append("-- Consolidated Initial Schema Migration")
    migration_sql.append("-- This migration consolidates all previous migrations into a single migration")
    migration_sql.append("-- Generated from target schema dump")
    migration_sql.append("")

    # Add extensions
    if statements['extensions']:
        migration_sql.append("-- Extensions")
        for ext in statements['extensions']:
            migration_sql.append(f"CREATE EXTENSION IF NOT EXISTS {ext} WITH SCHEMA public;")
        migration_sql.append("")

    # Add custom types
    if statements['types']:
        migration_sql.append("-- Custom Types (ENUMs)")
        for type_name, enum_values in statements['types']:
            migration_sql.append(f"CREATE TYPE public.{type_name} AS ENUM (")
            # Format enum values nicely
            values = [v.strip() for v in enum_values.split(',')]
            for i, value in enumerate(values):
                if i == len(values) - 1:
                    migration_sql.append(f"    {value}")
                else:
                    migration_sql.append(f"    {value},")
            migration_sql.append(");")
        migration_sql.append("")

    # Add tables
    if statements['tables']:
        migration_sql.append("-- Tables")
        for table_name, table_def in statements['tables']:
            migration_sql.append(f"CREATE TABLE public.{table_name} (")
            # Format table definition
            lines = table_def.strip().split('\n')
            for line in lines:
                migration_sql.append(f"    {line.strip()}")
            migration_sql.append(");")
        migration_sql.append("")

    # Add constraints
    if statements['constraints']:
        migration_sql.append("-- Constraints")
        for table_name, constraint_name, constraint_def in statements['constraints']:
            migration_sql.append(f"ALTER TABLE ONLY public.{table_name}")
            migration_sql.append(f"    ADD CONSTRAINT {constraint_name} {constraint_def};")
        migration_sql.append("")

    # Add indexes
    if statements['indexes']:
        migration_sql.append("-- Indexes")
        for index_name, table_name, index_type, columns, where_clause in statements['indexes']:
            where_part = f" WHERE {where_clause}" if where_clause else ""
            migration_sql.append(f"CREATE INDEX {index_name} ON public.{table_name} USING {index_type} ({columns}){where_part};")
        migration_sql.append("")

    # Add triggers
    if statements['triggers']:
        migration_sql.append("-- Triggers")
        for trigger_name, trigger_def in statements['triggers']:
            migration_sql.append(f"CREATE TRIGGER {trigger_name} {trigger_def};")
        migration_sql.append("")

    return '\n'.join(migration_sql)


def main():
    """Main function."""
    if len(sys.argv) != 2:
        print("Usage: python3 create_consolidated_migration.py <schema_file>")
        sys.exit(1)

    schema_file = sys.argv[1]
    if not Path(schema_file).exists():
        print(f"Schema file not found: {schema_file}")
        sys.exit(1)

    print(f"Processing schema file: {schema_file}")

    # Extract DDL statements
    statements = extract_ddl_statements(schema_file)

    print(f"Found:")
    print(f"  - {len(statements['extensions'])} extensions")
    print(f"  - {len(statements['types'])} custom types")
    print(f"  - {len(statements['tables'])} tables")
    print(f"  - {len(statements['indexes'])} indexes")
    print(f"  - {len(statements['constraints'])} constraints")
    print(f"  - {len(statements['triggers'])} triggers")
    print(f"  - {len(statements['functions'])} functions")

    # Generate consolidated migration
    migration_sql = generate_consolidated_migration(statements)

    # Write to file
    output_file = "backend/migrations/2025-02-01-000001_consolidated_initial_schema/up.sql"
    with open(output_file, 'w') as f:
        f.write(migration_sql)

    print(f"Consolidated migration written to: {output_file}")
    print(f"Migration size: {len(migration_sql)} characters")


if __name__ == "__main__":
    main()
