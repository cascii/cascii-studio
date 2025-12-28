# Versioning Scripts

This directory contains scripts for automatic semantic versioning based on commit message prefixes.

## How It Works

When you make a commit with a specific prefix, the version is automatically bumped:

| Commit Prefix | Version Bump | Example |
|--------------|--------------|---------|
| `fix(...): ...` | Patch (0.0.X) | `0.1.0` → `0.1.1` |
| `feature(...): ...` | Minor (0.X.0) | `0.1.0` → `0.2.0` |
| `release(...): ...` | Major (X.0.0) | `0.1.0` → `1.0.0` |

## Examples

```bash
# Bumps patch version (0.1.0 -> 0.1.1)
git commit -m "fix(video-player): resolve playback issue on Safari"

# Bumps minor version (0.1.1 -> 0.2.0)
git commit -m "feature(export): add MP4 export functionality"

# Bumps major version (0.2.0 -> 1.0.0)
git commit -m "release(v1): first stable release"

# No version bump
git commit -m "docs: update README"
git commit -m "chore: refactor code"
```

## Files Updated

The version bump script automatically updates:
- `src-tauri/Cargo.toml` - Rust package version
- `src-tauri/tauri.conf.json` - Tauri app version
- `src-tauri/Cargo.lock` - Updated automatically by cargo

## Installation

The git hook is already installed at `.git/hooks/prepare-commit-msg`.

If you need to reinstall it:

```bash
chmod +x .git/hooks/prepare-commit-msg
```

## Manual Version Bump

You can also run the script manually:

```bash
python3 scripts/bump_version.py "fix(player): bug fix"
python3 scripts/bump_version.py "feature(export): new feature"
python3 scripts/bump_version.py "release(v1): major release"
```

## Disabling Auto-Versioning

To temporarily disable auto-versioning, rename or remove the hook:

```bash
mv .git/hooks/prepare-commit-msg .git/hooks/prepare-commit-msg.disabled
```

To re-enable:

```bash
mv .git/hooks/prepare-commit-msg.disabled .git/hooks/prepare-commit-msg
```

