#!/usr/bin/env python3
"""
Automatic version bumping script for cascii_studio.
Bumps version in Cargo.toml and tauri.conf.json based on commit message prefix.

Usage:
    python3 scripts/bump_version.py <commit_message>

Commit message prefixes:
    release(...): ...   -> Bumps major version (X.0.0)
    feature(...): ...   -> Bumps minor version (0.X.0)
    fix(...): ...       -> Bumps patch version (0.0.X)
    anything else       -> No version bump
"""

import sys
import re
import json
from pathlib import Path

def parse_version(version_str):
    """Parse version string into (major, minor, patch) tuple.
    Supports 3-component SemVer (x.y.z) format.
    """
    # Remove any 4th component if present (for migration)
    version_str = re.sub(r'\.\d+$', '', version_str) if re.match(r'^\d+\.\d+\.\d+\.\d+', version_str) else version_str
    
    # Parse 3-component version
    match = re.match(r'(\d+)\.(\d+)\.(\d+)', version_str)
    if match:
        return tuple(map(int, match.groups()))
    
    raise ValueError(f"Invalid version format: {version_str}")

def bump_version(version_str, bump_type):
    """Bump version based on type: 'major', 'minor', or 'patch'.
    Returns 3-component SemVer (x.y.z) for both Cargo.toml and tauri.conf.json.
    """
    major, minor, patch = parse_version(version_str)
    
    if bump_type == 'major':
        return f"{major + 1}.0.0"
    elif bump_type == 'minor':
        return f"{major}.{minor + 1}.0"
    elif bump_type == 'patch':
        return f"{major}.{minor}.{patch + 1}"
    else:
        return version_str

def get_bump_type_from_commit(commit_msg):
    """Determine bump type from commit message.
    Only fix/feature/release commits will bump version.
    """
    commit_msg = commit_msg.strip().lower()
    
    if commit_msg.startswith('release('):
        return 'major'
    elif commit_msg.startswith('feature('):
        return 'minor'
    elif commit_msg.startswith('fix('):
        return 'patch'
    else:
        # Any other commit (cicd, docs, chore, etc.) - no version bump
        return None

def update_cargo_toml(file_path, new_version):
    """Update version in Cargo.toml - only the [package] section version."""
    content = file_path.read_text()
    
    # More precise regex: match [package] section, then find version line within that section
    # Stop at the next [section] or end of file
    lines = content.split('\n')
    updated_lines = []
    in_package_section = False
    version_updated = False
    
    for i, line in enumerate(lines):
        # Check if we're entering the [package] section
        if line.strip() == '[package]':
            in_package_section = True
            updated_lines.append(line)
            continue
        
        # Check if we're leaving the [package] section (entering another section)
        if in_package_section and line.strip().startswith('[') and line.strip() != '[package]':
            in_package_section = False
        
        # If we're in [package] section and this is the version line, update it
        if in_package_section and re.match(r'^\s*version\s*=\s*"[^"]+"', line):
            updated_lines.append(re.sub(r'(version\s*=\s*")[^"]+(")', rf'\g<1>{new_version}\g<2>', line))
            version_updated = True
        else:
            updated_lines.append(line)
    
    if version_updated:
        file_path.write_text('\n'.join(updated_lines))
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
    src_tauri_cargo_toml = root_dir / 'src-tauri' / 'Cargo.toml'
    root_cargo_toml = root_dir / 'Cargo.toml'
    tauri_conf = root_dir / 'src-tauri' / 'tauri.conf.json'
    
    # Get current version from tauri.conf.json
    with open(tauri_conf, 'r') as f:
        config = json.load(f)
    current_version = config['version']
    
    # Bump version - returns 3-component SemVer (x.y.z)
    new_version = bump_version(current_version, bump_type)
    
    if new_version == current_version:
        print(f"No version change: {current_version}")
        sys.exit(0)
    
    # Update files with same 3-component version format
    src_tauri_updated = update_cargo_toml(src_tauri_cargo_toml, new_version)
    root_updated = update_cargo_toml(root_cargo_toml, new_version)
    tauri_updated = update_tauri_conf(tauri_conf, new_version)
    
    if src_tauri_updated or root_updated or tauri_updated:
        print(f"Version bumped: {current_version} -> {new_version} ({bump_type})")
        if src_tauri_updated:
            print(f"Updated: {src_tauri_cargo_toml.relative_to(root_dir)}")
        if root_updated:
            print(f"Updated: {root_cargo_toml.relative_to(root_dir)}")
        if tauri_updated:
            print(f"Updated: {tauri_conf.relative_to(root_dir)}")
        sys.exit(0)
    else:
        print(f"No changes made for version: {current_version}")
        sys.exit(0)

if __name__ == '__main__':
    main()
