# Automatic Versioning System

This project uses automatic semantic versioning based on commit message prefixes, with **one version bump per PR/branch**.

## How It Works

When you make a commit with a specific prefix, the version is automatically bumped **once per PR/branch**:

| Commit Prefix | Version Bump | Example |
|--------------|--------------|---------|
| `fix(...): ...` | Patch (0.0.X.0) | `0.1.0` → `0.1.1.0` |
| `feature(...): ...` | Minor (0.X.0.0) | `0.1.0` → `0.2.0.0` |
| `release(...): ...` | Major (X.0.0.0) | `0.1.0` → `1.0.0.0` |
| **Any other commit** | Build (0.0.0.X) | `0.1.0` → `0.1.0.1` |

## Key Features

- ✅ **Automatic**: Runs on every commit via git hook
- ✅ **Once per PR**: Tracks branches to ensure only one bump per PR
- ✅ **Immediate**: Version increments when you commit, not when PR is merged
- ✅ **Smart**: Only bumps if commit message matches pattern
- ✅ **4-Component Support**: Supports build numbers (0.0.0.X) for CI/CD commits

## Examples

```bash
# First commit on feature branch - bumps version
git commit -m "feature(ascii): add conversion functionality"
# Version: 0.1.0 → 0.2.0.0

# Second commit on same branch - NO bump (already bumped)
git commit -m "fix(ui): fix button styling"
# Version: Still 0.2.0.0

# Switch to new branch - can bump again
git checkout -b fix/bug
git commit -m "fix(player): resolve playback issue"
# Version: 0.2.0.0 → 0.2.1.0

# Any other commit - bumps build number
git commit -m "docs(readme): update documentation"
# Version: 0.2.1.0 → 0.2.1.1

git commit -m "chore(deps): update dependencies"
# Version: 0.2.1.1 → 0.2.1.2

git commit -m "update something"
# Version: 0.2.1.2 → 0.2.1.3
```

## Branch Tracking

The system tracks which branches have been bumped in `.git/version_bumped_branches`. This file:
- Is automatically created and managed
- Is in `.gitignore` (not committed)
- Prevents multiple bumps on the same branch

## Resetting Version Tracking

If you need to reset version tracking (e.g., after merging a PR):

### Reset a specific branch:
```bash
./scripts/reset_version_tracking.sh feature/my-feature
```

### Reset all branches:
```bash
./scripts/reset_version_tracking.sh
```

## Manual Version Bump

You can also run the script manually:

```bash
python3 scripts/bump_version.py "fix(test): test"
python3 scripts/bump_version.py "feature(test): test"
python3 scripts/bump_version.py "release(test): test"
```

**Note**: Manual bumps don't update branch tracking, so the hook may still bump again.

## Workflow Example

1. **Create feature branch:**
   ```bash
   git checkout -b feature/add-conversion
   ```

2. **Make first commit with feature prefix:**
   ```bash
   git commit -m "feature(conversion): add ASCII conversion"
   # ✅ Version bumped: 0.1.0 → 0.2.0
   # ✅ Branch tracked: feature/add-conversion
   ```

3. **Make more commits (no more bumps):**
   ```bash
   git commit -m "fix(conversion): improve error handling"
   # ⏭️ No bump (branch already bumped)
   
   git commit -m "fix(ui): update button styles"
   # ⏭️ No bump (branch already bumped)
   ```

4. **After PR is merged:**
   ```bash
   # Reset tracking for the branch (optional, for cleanup)
   ./scripts/reset_version_tracking.sh feature/add-conversion
   ```

## Files Updated

The version bump automatically updates:
- `src-tauri/Cargo.toml` - Rust package version
- `src-tauri/tauri.conf.json` - Tauri app version
- `src-tauri/Cargo.lock` - Updated automatically by cargo

## Troubleshooting

### Version not bumping?

1. **Check commit message format:**
   - Must start with `fix(`, `feature(`, or `release(`
   - Must have parentheses and colon: `fix(...): description`

2. **Check if branch already bumped:**
   ```bash
   cat .git/version_bumped_branches
   ```

3. **Reset if needed:**
   ```bash
   ./scripts/reset_version_tracking.sh your-branch-name
   ```

### Hook not running?

Check if the hook is executable:
```bash
ls -l .git/hooks/prepare-commit-msg
```

If not, make it executable:
```bash
chmod +x .git/hooks/prepare-commit-msg
```

### Disable temporarily

To temporarily disable auto-versioning:
```bash
mv .git/hooks/prepare-commit-msg .git/hooks/prepare-commit-msg.disabled
```

To re-enable:
```bash
mv .git/hooks/prepare-commit-msg.disabled .git/hooks/prepare-commit-msg
```

## See Also

- [scripts/README.md](README.md) - Technical details
- [Semantic Versioning](https://semver.org/) - Versioning standard
- [Conventional Commits](https://www.conventionalcommits.org/) - Commit message convention

