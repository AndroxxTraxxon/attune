#!/bin/sh
# Post-install script for Attune packages
set -e

# Create attune system user and group if they don't exist
if ! getent group attune >/dev/null 2>&1; then
    groupadd --system attune
fi

if ! getent passwd attune >/dev/null 2>&1; then
    useradd --system --gid attune --home-dir /var/lib/attune \
        --shell /usr/sbin/nologin --comment "Attune automation platform" attune
fi

# Create required directories
for dir in /var/lib/attune /var/lib/attune/packs /var/lib/attune/runtime_envs \
           /var/lib/attune/artifacts /var/lib/attune/agent /var/log/attune; do
    mkdir -p "$dir"
    chown attune:attune "$dir"
    chmod 750 "$dir"
done

# Ensure config directory exists and has correct permissions
mkdir -p /etc/attune
chmod 750 /etc/attune

# Config files should be readable by attune group
if [ -f /etc/attune/attune.yaml ]; then
    chown root:attune /etc/attune/attune.yaml
    chmod 640 /etc/attune/attune.yaml
fi

if [ -f /etc/attune/environment ]; then
    chown root:attune /etc/attune/environment
    chmod 640 /etc/attune/environment
fi

# Reload systemd if a service unit was installed
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload || true
fi

echo ""
echo "Attune installed successfully."
echo ""
echo "Next steps:"
echo "  1. Edit /etc/attune/environment to set JWT_SECRET and ENCRYPTION_KEY"
echo "  2. Edit /etc/attune/attune.yaml to configure database and RabbitMQ URLs"
echo "  3. Run database migrations: attune-api --migrate (or use sqlx-cli)"
echo "  4. Enable and start services:"
echo "     systemctl enable --now attune-api attune-executor attune-supervisor attune-notifier"
echo ""
