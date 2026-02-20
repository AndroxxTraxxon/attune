# Core Pack Runtime Metadata

This directory contains runtime metadata YAML files for the core pack. Each file defines a runtime environment that can be used to execute actions and sensors.

## File Structure

Each runtime YAML file contains only the fields that are stored in the database:

- `ref` - Unique runtime reference (format: pack.name)
- `pack_ref` - Pack this runtime belongs to
- `name` - Human-readable runtime name
- `description` - Brief description of the runtime
- `distributions` - Runtime verification and capability metadata (JSONB)
- `installation` - Installation requirements and metadata (JSONB)

## Available Runtimes

- **python.yaml** - Python 3 runtime for actions and sensors
- **nodejs.yaml** - Node.js runtime for JavaScript-based actions and sensors
- **shell.yaml** - Shell (bash/sh) runtime - always available
- **native.yaml** - Native compiled runtime (Rust, Go, C, etc.) - executes binaries directly without an interpreter

## Loading

Runtime metadata files are loaded by the pack loading system and inserted into the `runtime` table in the database.
