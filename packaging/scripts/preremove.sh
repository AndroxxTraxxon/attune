#!/bin/sh
# Pre-remove script for Attune packages
set -e

# Stop services before removal
if command -v systemctl >/dev/null 2>&1; then
    for svc in attune-api attune-executor attune-supervisor attune-notifier; do
        if systemctl is-active --quiet "$svc" 2>/dev/null; then
            systemctl stop "$svc" || true
        fi
        if systemctl is-enabled --quiet "$svc" 2>/dev/null; then
            systemctl disable "$svc" || true
        fi
    done
fi
