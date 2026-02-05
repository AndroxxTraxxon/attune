# Quick Reference: Development Packs

## Setup (One Time)

```bash
# Directory is already created, just start Docker
docker compose up -d
```

## Create a Pack

```bash
./scripts/dev-pack.sh create my-pack
```

Creates:
- `packs.dev/my-pack/pack.yaml`
- `packs.dev/my-pack/actions/example.sh`
- Example action YAML
- README

## List Packs

```bash
./scripts/dev-pack.sh list
```

## Validate Pack

```bash
./scripts/dev-pack.sh validate my-pack
```

Checks:
- ✓ pack.yaml exists
- ✓ Action scripts exist and are executable
- ✓ Entry points match

## Register Pack in Attune

```bash
# Get token first
export ATTUNE_TOKEN=$(attune auth login test@attune.local --password TestPass123!)

# Register pack
curl -X POST http://localhost:8080/api/v1/packs \
  -H "Authorization: Bearer $ATTUNE_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "my-pack",
    "label": "My Pack",
    "version": "1.0.0",
    "enabled": true
  }'
```

## Execute Action

```bash
curl -X POST http://localhost:8080/api/v1/executions \
  -H "Authorization: Bearer $ATTUNE_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "action": "my-pack.example",
    "parameters": {
      "message": "Hello!"
    }
  }'
```

## Directory Layout

```
packs.dev/
├── examples/              # Example packs (in git)
│   ├── basic-pack/       # Shell action example
│   └── python-pack/      # Python action example
└── my-pack/              # Your packs (not in git)
    ├── pack.yaml
    ├── actions/
    ├── triggers/
    ├── sensors/
    └── workflows/
```

## File Locations in Docker

- **Core pack**: `/opt/attune/packs` (read-only)
- **Dev packs**: `/opt/attune/packs.dev` (read-write)

## Development Workflow

1. Create pack: `./scripts/dev-pack.sh create my-pack`
2. Edit files: `vim packs.dev/my-pack/actions/my_action.sh`
3. Validate: `./scripts/dev-pack.sh validate my-pack`
4. Register: See "Register Pack" above
5. Test: Execute action via API
6. Iterate: Changes are immediately visible!

## Action Script Template

```bash
#!/bin/bash
set -e

# Get parameters from environment
PARAM="${ATTUNE_ACTION_param:-default}"

# Validate
if [ -z "$PARAM" ]; then
    echo '{"error": "param required"}' >&2
    exit 1
fi

# Do work
result=$(echo "Processed: $PARAM")

# Return JSON
echo "{\"result\": \"$result\"}"
```

## Common Commands

```bash
# List all packs
./scripts/dev-pack.sh list

# Validate pack structure
./scripts/dev-pack.sh validate my-pack

# View pack in container
docker exec attune-api ls -la /opt/attune/packs.dev/

# Check worker logs
docker logs -f attune-worker-shell

# Sync workflows after changes
curl -X POST http://localhost:8080/api/v1/packs/my-pack/workflows/sync \
  -H "Authorization: Bearer $ATTUNE_TOKEN"

# Clean up dev packs
./scripts/dev-pack.sh clean
```

## Troubleshooting

### "Pack not found"
```bash
# Check if registered
curl http://localhost:8080/api/v1/packs/my-pack \
  -H "Authorization: Bearer $ATTUNE_TOKEN"

# Check if files exist in container
docker exec attune-api ls /opt/attune/packs.dev/my-pack/
```

### "Entry point not found"
```bash
# Make script executable
chmod +x packs.dev/my-pack/actions/*.sh

# Verify in container
docker exec attune-worker-shell ls -la /opt/attune/packs.dev/my-pack/actions/
```

### Changes not reflected
```bash
# For action scripts: should be immediate
# For action YAML: re-register pack
# For workflows: run sync endpoint
```

## See Also

- [Full Documentation](development/packs-dev-directory.md)
- [Pack Structure](packs/pack-structure.md)
- [Examples](../packs.dev/examples/)
