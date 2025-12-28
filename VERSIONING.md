# Automatic Versioning Guide

This project uses **automatic semantic versioning** based on git commit message prefixes.

## Quick Reference

| Commit Prefix | Bump Type | Version Change | Use Case |
|--------------|-----------|----------------|----------|
| `fix(...):` | **Patch** | 0.0.X | Bug fixes, small corrections |
| `feature(...):` | **Minor** | 0.X.0 | New features, enhancements |
| `release(...):` | **Major** | X.0.0 | Breaking changes, major releases |
| Other | None | No change | Documentation, refactoring, etc. |

## Examples

### Bug Fixes (Patch: 0.0.X)
```bash
git commit -m "fix(video-player): resolve playback stuttering"
git commit -m "fix(database): prevent duplicate entries"
git commit -m "fix(ui): correct button alignment"
```
**Result**: `0.1.0` → `0.1.1`

### New Features (Minor: 0.X.0)
```bash
git commit -m "feature(export): add MP4 export functionality"
git commit -m "feature(timeline): implement frame scrubbing"
git commit -m "feature(import): support WebM format"
```
**Result**: `0.1.1` → `0.2.0`

### Major Releases (Major: X.0.0)
```bash
git commit -m "release(v1): first stable release"
git commit -m "release(v2): complete UI redesign"
git commit -m "release(api): breaking API changes"
```
**Result**: `0.2.0` → `1.0.0`

### No Version Bump
```bash
git commit -m "docs: update README with examples"
git commit -m "chore: refactor code structure"
git commit -m "style: format code with rustfmt"
git commit -m "test: add unit tests"
```
**Result**: Version stays the same

## How It Works

1. **You commit** with a special prefix (e.g., `fix(...)`, `feature(...)`, `release(...)`)
2. **Git hook runs** before the commit is finalized (`.git/hooks/prepare-commit-msg`)
3. **Script executes** (`scripts/bump_version.py`) and determines the bump type
4. **Files updated**:
   - `src-tauri/Cargo.toml` - Rust package version
   - `src-tauri/tauri.conf.json` - Tauri app version
   - `src-tauri/Cargo.lock` - Automatically updated by cargo
5. **Changes staged** and included in your commit automatically

## Advanced Usage

### Manual Version Bump

If you need to bump the version manually without committing:

```bash
python3 scripts/bump_version.py "fix(manual): manual bump"
python3 scripts/bump_version.py "feature(manual): manual bump"
python3 scripts/bump_version.py "release(manual): manual bump"
```

### Check Current Version

```bash
grep '"version"' src-tauri/tauri.conf.json
# or
grep '^version' src-tauri/Cargo.toml
```

### Temporarily Disable Auto-Versioning

```bash
# Disable
mv .git/hooks/prepare-commit-msg .git/hooks/prepare-commit-msg.disabled

# Re-enable
mv .git/hooks/prepare-commit-msg.disabled .git/hooks/prepare-commit-msg
```

### Skip Version Bump for a Specific Commit

Use any prefix other than `fix()`, `feature()`, or `release()`:

```bash
git commit -m "wip: work in progress"
git commit -m "temp: temporary changes"
```

## Best Practices

### 1. **Use Descriptive Scopes**

Good:
```bash
git commit -m "fix(video-player): resolve audio sync issue"
git commit -m "feature(timeline): add zoom controls"
```

Bad:
```bash
git commit -m "fix(stuff): fixed things"
git commit -m "feature(app): new stuff"
```

### 2. **Be Consistent**

- Use `fix()` for all bug fixes
- Use `feature()` for all new features
- Use `release()` only for major milestones

### 3. **Group Related Changes**

Instead of:
```bash
git commit -m "fix(ui): button style"
git commit -m "fix(ui): input style"
git commit -m "fix(ui): layout"
```

Do:
```bash
# Make all changes, then
git commit -m "fix(ui): correct styling issues for buttons, inputs, and layout"
```

### 4. **Test Before Release**

Before using `release()`:
1. Ensure all tests pass
2. Review all changes since last major version
3. Update documentation
4. Create a changelog

## Troubleshooting

### Hook Not Running

Check if the hook is executable:
```bash
ls -l .git/hooks/prepare-commit-msg
```

If not, make it executable:
```bash
chmod +x .git/hooks/prepare-commit-msg
```

### Script Errors

Test the script manually:
```bash
python3 scripts/bump_version.py "fix(test): test"
```

If you get errors, ensure Python 3 is installed:
```bash
python3 --version
```

### Version Not Updating

1. Check commit message format (must start with `fix(`, `feature(`, or `release(`)
2. Ensure parentheses and colon are present: `fix(...): description`
3. Check that files aren't read-only

## See Also

- [scripts/README.md](scripts/README.md) - Technical details
- [Semantic Versioning](https://semver.org/) - Versioning standard
- [Conventional Commits](https://www.conventionalcommits.org/) - Commit message convention

