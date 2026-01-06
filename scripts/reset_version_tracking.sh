#!/bin/bash
# Reset version tracking for a branch or all branches
# Usage:
#   ./scripts/reset_version_tracking.sh [branch_name]
#   If branch_name is provided, only that branch is reset
#   If no branch_name, all tracking is reset

REPO_ROOT=$(git rev-parse --show-toplevel)
VERSION_TRACK_FILE="$REPO_ROOT/.git/version_bumped_branches"

if [ ! -f "$VERSION_TRACK_FILE" ]; then
    echo "No version tracking file found. Nothing to reset."
    exit 0
fi

if [ -z "$1" ]; then
    # Reset all branches
    echo "Resetting version tracking for all branches..."
    rm "$VERSION_TRACK_FILE"
    echo "✅ All version tracking reset"
else
    # Reset specific branch
    BRANCH_NAME="$1"
    echo "Resetting version tracking for branch: $BRANCH_NAME"
    
    if grep -q "^$BRANCH_NAME$" "$VERSION_TRACK_FILE" 2>/dev/null; then
        # Remove the branch from tracking file
        grep -v "^$BRANCH_NAME$" "$VERSION_TRACK_FILE" > "$VERSION_TRACK_FILE.tmp" 2>/dev/null
        mv "$VERSION_TRACK_FILE.tmp" "$VERSION_TRACK_FILE"
        echo "✅ Version tracking reset for branch: $BRANCH_NAME"
    else
        echo "Branch '$BRANCH_NAME' was not found in version tracking."
    fi
fi
