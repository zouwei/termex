#!/bin/bash
# ~/.termex/git-sync.sh — Termex Auto Git Sync
# Usage: codex "task" && ~/.termex/git-sync.sh [path]
#        aider --message "fix" && ~/.termex/git-sync.sh
#        any_command && ~/.termex/git-sync.sh

cd "${1:-.}" || exit 1

# Check for changes
if git diff --quiet HEAD 2>/dev/null && [ -z "$(git ls-files --others --exclude-standard)" ]; then
    echo "[termex-sync] No changes to commit"
    exit 0
fi

# Auto commit + push
git add -A
git commit -m "auto: $(date +%Y%m%d-%H%M%S)"
SYNC_PORT=$(cat ~/.termex/sync-port 2>/dev/null || echo 19527)
if git push; then
    echo "[termex-sync] Push successful"
    # Notify local Termex via reverse tunnel (silent on failure)
    curl -s -m 2 "http://127.0.0.1:${SYNC_PORT}/push-done" 2>/dev/null || true
else
    echo "[termex-sync] Push failed"
    exit 1
fi
