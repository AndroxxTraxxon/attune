#!/bin/sh
# Post-remove script for Attune packages
set -e

# Reload systemd after unit file removal
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload || true
fi

# On purge (dpkg --purge), clean up data directories
# RPM doesn't have a "purge" concept, so this only runs for deb
if [ "$1" = "purge" ]; then
    rm -rf /var/lib/attune
    rm -rf /var/log/attune
    rm -rf /etc/attune
    rm -rf /opt/attune-system

    # Remove attune user and group
    if getent passwd attune >/dev/null 2>&1; then
        userdel attune || true
    fi
    if getent group attune >/dev/null 2>&1; then
        groupdel attune || true
    fi
fi
