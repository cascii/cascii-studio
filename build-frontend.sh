#!/bin/bash
# Cross-platform script to build frontend with trunk
# This handles the NO_COLOR environment variable issue

if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
    # Windows
    set NO_COLOR=
    trunk build
else
    # Unix-like systems (Linux, macOS)
    unset NO_COLOR
    trunk build
fi
