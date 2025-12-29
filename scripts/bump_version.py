#!/usr/bin/env python3
"""
Automatic version bumping script for cascii_studio.
Bumps version in Cargo.toml and tauri.conf.json based on commit message prefix.

Usage:
    python3 scripts/bump_version.py <commit_message>

Commit message prefixes:
    fix(...): ...       -> Bumps patch version (0.0.X.0)
    feature(...): ...   -> Bumps minor version (0.X.0.0)
    release(...): ...   -> Bumps major version (X.0.0.0)
    anything else       -> Bumps build version (0.0.0.X)
"""

import sys
import re
import json
from pathlib import Path


def parse_version(version_str):
    """Parse version string into (major, minor, patch, build) tuple.
    Supports both 3-component (0.1.0) and 4-component (0.1.0.1) versions.
    """
    # Try 4-component first
    match = re.match(r'(\d+)\.(\d+)\.(\d+)\.(\d+)', version_str)
    if match:
        return tuple(map(int, match.groups()))
    
    # Fall back to 3-component
    match = re.match(r'(\d+)\.(\d+)\.(\d+)', version_str)
    if match:
        major, minor, patch = map(int, match.groups())
        return (major, minor, patch, 0)  # Default build to 0
    
    raise ValueError(f"Invalid version format: {version_str}")


def bump_version(version_str, bump_type):
    """Bump version based on type: 'major', 'minor', 'patch', or 'build'."""
    major, minor, patch, build = parse_version(version_str)
    
    if bump_type == 'major':
        return f"{major + 1}.0.0.0"
    elif bump_type == 'minor':
        return f"{major}.{minor + 1}.0.0"
    elif bump_type == 'patch':
        return f"{major}.{minor}.{patch + 1}.0"
    elif bump_type == 'build':
        return f"{major}.{minor}.{patch}.{build + 1}"
    else:
        return version_str


def get_bump_type_from_commit(commit_msg):
    """Determine bump type from commit message.
    Any commit that doesn't match fix/feature/release will bump build number.
    """
    commit_msg = commit_msg.strip().lower()
    
    if commit_msg.startswith('fix('):
        return 'patch'
    elif commit_msg.startswith('feature('):
        return 'minor'
    elif commit_msg.startswith('release('):
        return 'major'
    else:
        # Any other commit (cicd, docs, chore, etc.) bumps build number
        return 'build'


def update_cargo_toml(file_path, new_version):
    """Update version in Cargo.toml."""
    content = file_path.read_text()
    
    # Replace version in [package] section
    updated = re.sub(
        r'(^\[package\].*?^version\s*=\s*")[^"]+(")',
        rf'\g<1>{new_version}\g<2>',
        content,
        flags=re.MULTILINE | re.DOTALL
    )
    
    if updated != content:
        file_path.write_text(updated)
        return True
    return False


def update_tauri_conf(file_path, new_version):
    """Update version in tauri.conf.json."""
    with open(file_path, 'r') as f:
        config = json.load(f)
    
    old_version = config.get('version')
    config['version'] = new_version
    
    with open(file_path, 'w') as f:
        json.dump(config, f, indent=2)
        f.write('\n')  # Add trailing newline
    
    return old_version != new_version


def main():
    if len(sys.argv) < 2:
        print("Usage: bump_version.py <commit_message>", file=sys.stderr)
        sys.exit(1)
    
    commit_msg = sys.argv[1]
    bump_type = get_bump_type_from_commit(commit_msg)
    
    if not bump_type:
        # No version bump needed
        sys.exit(0)
    
    # Paths
    root_dir = Path(__file__).parent.parent
    cargo_toml = root_dir / 'src-tauri' / 'Cargo.toml'
    tauri_conf = root_dir / 'src-tauri' / 'tauri.conf.json'
    
    # Get current version from tauri.conf.json
    with open(tauri_conf, 'r') as f:
        config = json.load(f)
    current_version = config['version']
    
    # Bump version
    new_version = bump_version(current_version, bump_type)
    
    if new_version == current_version:
        print(f"No version change: {current_version}")
        sys.exit(0)
    
    # Update files
    cargo_updated = update_cargo_toml(cargo_toml, new_version)
    tauri_updated = update_tauri_conf(tauri_conf, new_version)
    
    if cargo_updated or tauri_updated:
        print(f"Version bumped: {current_version} -> {new_version} ({bump_type})")
        print(f"Updated: {cargo_toml.relative_to(root_dir)}")
        print(f"Updated: {tauri_conf.relative_to(root_dir)}")
        sys.exit(0)
    else:
        print(f"No changes made for version: {current_version}")
        sys.exit(0)


if __name__ == '__main__':
    main()

