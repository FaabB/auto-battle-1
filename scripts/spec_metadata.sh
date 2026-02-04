#!/bin/bash

# Script to gather metadata for thoughts documents
# Usage: ./scripts/spec_metadata.sh

# Get current date and time
CURRENT_DATE=$(date +"%Y-%m-%d")
CURRENT_TIME=$(date +"%H-%M-%S")
CURRENT_DATETIME=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
CURRENT_DATETIME_TZ=$(date +"%Y-%m-%dT%H:%M:%S%z")

# Get git information
GIT_COMMIT=$(git rev-parse HEAD 2>/dev/null || echo "not-a-git-repo")
GIT_BRANCH=$(git branch --show-current 2>/dev/null || echo "unknown")
REPO_NAME=$(basename "$(git rev-parse --show-toplevel 2>/dev/null)" || basename "$(pwd)")

# Get researcher name (from git config or environment)
RESEARCHER=$(git config user.name 2>/dev/null || echo "${USER:-unknown}")

# Output metadata
cat << EOF
=== Metadata for Thoughts Document ===

Current Date: ${CURRENT_DATE}
Current Time: ${CURRENT_TIME}
Current DateTime (UTC): ${CURRENT_DATETIME}
Current DateTime (TZ): ${CURRENT_DATETIME_TZ}

Git Commit: ${GIT_COMMIT}
Git Branch: ${GIT_BRANCH}
Repository: ${REPO_NAME}

Researcher: ${RESEARCHER}

Timestamp for filename: ${CURRENT_DATE}_${CURRENT_TIME}

=== End Metadata ===
EOF
