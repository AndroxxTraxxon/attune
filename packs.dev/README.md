# Development Packs Directory

This directory is for developing and testing custom packs outside of the core pack. Packs placed here are automatically available in Docker containers.

## Usage

### 1. Create a New Pack

```bash
cd packs.dev
mkdir my-pack
cd my-pack
```

### 2. Create pack.yaml

```yaml
ref: my-pack
label: "My Custom Pack"
description: "My custom automation pack"
version: "1.0.0"
author: "Your Name"
email: "you@example.com"

# Pack configuration
system: false
enabled: true
```

### 3. Add Actions

```bash
mkdir actions
cat > actions/hello.yaml << 'YAML'
name: hello
ref: my-pack.hello
description: "Say hello"
runner_type: shell
enabled: true
entry_point: hello.sh
parameters:
  type: object
  properties:
    name:
      type: string
      description: "Name to greet"
      default: "World"
  required: []
output:
  type: object
  properties:
    message:
      type: string
      description: "Greeting message"
YAML

cat > actions/hello.sh << 'BASH'
#!/bin/bash
echo "{\"message\": \"Hello, ${ATTUNE_ACTION_name}!\"}"
BASH

chmod +x actions/hello.sh
```

### 4. Access in Docker

The pack will be automatically available at `/opt/attune/packs.dev/my-pack` in all containers.

To load the pack into the database:

```bash
# Via API
curl -X POST http://localhost:8080/api/v1/packs \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "ref": "my-pack",
    "label": "My Custom Pack",
    "description": "My custom automation pack",
    "version": "1.0.0",
    "system": false,
    "enabled": true,
    "author": "Your Name",
    "email": "you@example.com"
  }'

# Or via CLI
attune pack register /opt/attune/packs.dev/my-pack
```

## Development Workflow

1. **Create pack structure** in `packs.dev/`
2. **Edit files** on your host machine
3. **Changes are immediately visible** in containers (bind mount)
4. **Test** by creating rules/workflows that use your pack
5. **Iterate** without rebuilding containers

## Directory Structure

```
packs.dev/
├── README.md (this file)
└── my-pack/
    ├── pack.yaml
    ├── actions/
    │   ├── my_action.yaml
    │   └── my_action.sh
    ├── triggers/
    │   └── my_trigger.yaml
    ├── sensors/
    │   └── my_sensor.yaml
    └── workflows/
        └── my_workflow.yaml
```

## Important Notes

- This directory is for **development only**
- Production packs should be properly packaged and installed
- Files are mounted **read-write** so be careful with modifications from containers
- The core pack is in `/opt/attune/packs` (read-only in containers)
- Dev packs are in `/opt/attune/packs.dev` (read-write in containers)

## Example Packs

See the `examples/` subdirectory for starter pack templates:
- `examples/basic-pack/` - Minimal pack with shell action
- `examples/python-pack/` - Pack with Python actions
- `examples/workflow-pack/` - Pack with workflows

## Troubleshooting

### Pack not found
- Ensure `pack.yaml` exists and is valid
- Check pack ref matches directory name (recommended)
- Verify pack is registered in database via API

### Actions not executing
- Check `entry_point` matches actual file name
- Ensure scripts are executable (`chmod +x`)
- Check action runner_type matches script type
- View worker logs: `docker logs attune-worker-shell`

### Permission errors
- Ensure files are readable by container user (UID 1000)
- Check file permissions: `ls -la packs.dev/my-pack/`

## See Also

- [Pack Structure Documentation](../docs/packs/pack-structure.md)
- [Action Development Guide](../docs/actions/action-development.md)
- [Workflow Development Guide](../docs/workflows/workflow-development.md)
