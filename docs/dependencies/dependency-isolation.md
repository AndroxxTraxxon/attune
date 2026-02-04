# Dependency Isolation

## Overview

The Attune Worker Service provides **dependency isolation** for pack-specific runtime dependencies. This critical feature prevents dependency conflicts between packs by creating isolated virtual environments for each pack that declares dependencies.

## Why Dependency Isolation?

In a multi-tenant automation platform, different packs may require:
- **Conflicting versions** of the same library (Pack A needs `requests==2.28.0`, Pack B needs `requests==2.31.0`)
- **Different Python versions** (Pack A requires Python 3.8, Pack B requires Python 3.11)
- **Incompatible dependencies** that would break if installed in the same environment

Without isolation, these conflicts would cause:
- ❌ Actions failing due to wrong dependency versions
- ❌ Security vulnerabilities from outdated dependencies
- ❌ Unpredictable behavior from version mismatches
- ❌ Production outages from dependency upgrades

With isolation, each pack gets:
- ✅ Its own virtual environment with exact dependencies
- ✅ Independence from other packs' requirements
- ✅ Reproducible execution environments
- ✅ Safe concurrent execution of actions from different packs

## Architecture

### Components

1. **DependencyManager Trait** (`runtime/dependency.rs`)
   - Generic interface for managing runtime dependencies
   - Extensible to multiple languages (Python, Node.js, Java, etc.)
   - Provides environment lifecycle management

2. **PythonVenvManager** (`runtime/python_venv.rs`)
   - Implements `DependencyManager` for Python
   - Creates and manages Python virtual environments (venv)
   - Handles dependency installation via pip
   - Caches environment information for performance

3. **DependencyManagerRegistry** (`runtime/dependency.rs`)
   - Central registry for all dependency managers
   - Routes pack dependencies to appropriate manager
   - Supports multiple runtime types simultaneously

4. **Python Runtime Integration** (`runtime/python.rs`)
   - Automatically uses pack-specific venv when available
   - Falls back to default Python interpreter for packs without dependencies
   - Transparent to action execution logic

### Workflow

```
Pack Installation/Update
    ↓
Check pack metadata for dependencies
    ↓
If dependencies declared:
    ↓
Create/update virtual environment
    ↓
Install dependencies via pip
    ↓
Cache environment info
    ↓
Action Execution
    ↓
Extract pack_ref from action_ref
    ↓
Get pack-specific Python executable
    ↓
Execute action in isolated environment
```

## Usage

### Declaring Dependencies in Packs

Dependencies are stored in the pack's `meta` JSONB field using the `python_dependencies` key:

```json
{
  "meta": {
    "python_dependencies": {
      "runtime": "python",
      "dependencies": [
        "requests==2.31.0",
        "pydantic>=2.0.0",
        "boto3~=1.28.0"
      ],
      "min_version": "3.8",
      "max_version": "3.11"
    }
  }
}
```

**Alternative: Requirements File**

```json
{
  "meta": {
    "python_dependencies": {
      "runtime": "python",
      "requirements_file_content": "requests==2.31.0\npydantic>=2.0.0\nboto3~=1.28.0"
    }
  }
}
```

### API Integration

When creating or updating a pack via the API:

```bash
POST /api/packs
Content-Type: application/json

{
  "ref": "aws.s3",
  "label": "AWS S3 Pack",
  "version": "1.0.0",
  "meta": {
    "python_dependencies": {
      "runtime": "python",
      "dependencies": [
        "boto3==1.28.0",
        "botocore==1.31.0"
      ]
    }
  }
}
```

### Worker Service Integration

The worker service automatically sets up dependency isolation on startup:

```rust
// In worker service initialization
let venv_base_dir = PathBuf::from("/tmp/attune/venvs");
let python_venv_manager = PythonVenvManager::new(venv_base_dir);

let mut dependency_registry = DependencyManagerRegistry::new();
dependency_registry.register(Box::new(python_venv_manager));

let python_runtime = PythonRuntime::with_dependency_manager(
    python_path,
    work_dir,
    Arc::new(dependency_registry),
);
```

### Manual Environment Management

#### Ensure Environment

```rust
use attune_worker::runtime::{DependencyManager, DependencySpec, PythonVenvManager};

let manager = PythonVenvManager::new(PathBuf::from("/tmp/attune/venvs"));
let spec = DependencySpec::new("python")
    .with_dependency("requests==2.31.0")
    .with_dependency("flask>=2.3.0");

let env_info = manager.ensure_environment("my_pack", &spec).await?;
println!("Environment ready at: {:?}", env_info.path);
```

#### List Environments

```rust
let environments = manager.list_environments().await?;
for env in environments {
    println!("{}: {} ({})", env.id, env.runtime_version, env.path.display());
}
```

#### Remove Environment

```rust
manager.remove_environment("my_pack").await?;
```

## Configuration

### Environment Variables

- `ATTUNE__WORKER__VENV_BASE_DIR` - Base directory for virtual environments (default: `/tmp/attune/venvs`)
- `ATTUNE__WORKER__PYTHON_PATH` - Python interpreter path (default: `python3`)

### Directory Structure

```
/tmp/attune/venvs/
├── pack_a/                    # Virtual environment for pack_a
│   ├── bin/python             # Isolated Python executable
│   ├── lib/python3.x/         # Installed packages
│   └── attune_metadata.json   # Environment metadata
├── pack_b/
│   ├── bin/python
│   ├── lib/python3.x/
│   └── attune_metadata.json
└── core_http/                 # Sanitized name for core.http
    ├── bin/python
    ├── lib/python3.x/
    └── attune_metadata.json
```

### Metadata File Format

Each environment has an `attune_metadata.json` file:

```json
{
  "pack_ref": "aws.s3",
  "dependencies": ["boto3==1.28.0", "botocore==1.31.0"],
  "created_at": "2025-01-27T10:00:00Z",
  "updated_at": "2025-01-27T10:00:00Z",
  "python_version": "Python 3.10.12",
  "dependency_hash": "a1b2c3d4e5f6"
}
```

## Performance Considerations

### Caching

- **Environment Info Cache**: Metadata is cached in memory to avoid repeated disk I/O
- **Idempotent Operations**: `ensure_environment()` checks dependency hash before recreating
- **Lazy Creation**: Environments are only created when first needed

### Update Detection

The system uses dependency hash comparison to detect changes:
- If hash matches → Use existing environment
- If hash differs → Recreate environment with new dependencies

### Cleanup

```rust
// Remove old/invalid environments, keeping 10 most recent
let removed = manager.cleanup(10).await?;
```

## Security

### Isolation Benefits

1. **No Global Pollution**: Dependencies don't leak between packs
2. **Version Pinning**: Exact versions ensure reproducibility
3. **Sandboxing**: Each pack runs in its own environment
4. **Audit Trail**: Metadata tracks what's installed

### Best Practices

- ✅ Pin exact dependency versions (`requests==2.31.0`)
- ✅ Use virtual environments (automatically handled)
- ✅ Validate environment before execution
- ✅ Regularly update dependencies in pack definitions
- ❌ Don't use wildcard versions (`requests==*`)
- ❌ Don't install system-wide packages

## Troubleshooting

### Environment Creation Fails

**Problem**: `Failed to create Python virtual environment`

**Solution**:
1. Ensure Python is installed: `python3 --version`
2. Ensure venv module is available: `python3 -m venv --help`
3. Check disk space in venv base directory
4. Verify write permissions

### Dependency Installation Fails

**Problem**: `pip install failed: Could not find version`

**Solution**:
1. Verify dependency name and version exist on PyPI
2. Check network connectivity
3. Review `stderr` in error message for details
4. Try installing manually: `pip install <package>==<version>`

### Wrong Python Version Used

**Problem**: Action uses default Python instead of venv

**Solution**:
1. Verify pack has `python_dependencies` in metadata
2. Check `ensure_environment()` was called before execution
3. Verify venv was created successfully
4. Check worker logs for "Using pack-specific Python from venv"

### Environment Invalid

**Problem**: `Environment validation failed`

**Solution**:
1. Remove invalid environment: `manager.remove_environment("pack_ref").await`
2. Re-create: `manager.ensure_environment("pack_ref", &spec).await`
3. Check Python executable exists in venv
4. Verify venv structure is intact

## Future Enhancements

### Node.js Support

```rust
pub struct NodeVenvManager {
    base_dir: PathBuf,
    node_path: PathBuf,
}

impl DependencyManager for NodeVenvManager {
    fn runtime_type(&self) -> &str { "nodejs" }
    // ... implementation using npm/yarn
}
```

### Java Support

```rust
pub struct MavenDependencyManager {
    base_dir: PathBuf,
    maven_repo: PathBuf,
}

impl DependencyManager for MavenDependencyManager {
    fn runtime_type(&self) -> &str { "java" }
    // ... implementation using Maven
}
```

### Container-Based Isolation

For even stronger isolation:
- Each pack gets a Docker container
- Pre-built images with dependencies
- Resource limits and security policies

### Dependency Caching

- Shared pip cache across environments
- Pre-downloaded common packages
- Faster environment creation

## API Reference

### DependencyManager Trait

```rust
#[async_trait]
pub trait DependencyManager: Send + Sync {
    fn runtime_type(&self) -> &str;
    
    async fn ensure_environment(
        &self,
        pack_ref: &str,
        spec: &DependencySpec,
    ) -> DependencyResult<EnvironmentInfo>;
    
    async fn get_environment(&self, pack_ref: &str) 
        -> DependencyResult<Option<EnvironmentInfo>>;
    
    async fn remove_environment(&self, pack_ref: &str) 
        -> DependencyResult<()>;
    
    async fn validate_environment(&self, pack_ref: &str) 
        -> DependencyResult<bool>;
    
    async fn get_executable_path(&self, pack_ref: &str) 
        -> DependencyResult<PathBuf>;
    
    async fn list_environments(&self) 
        -> DependencyResult<Vec<EnvironmentInfo>>;
    
    async fn cleanup(&self, keep_recent: usize) 
        -> DependencyResult<Vec<String>>;
}
```

### DependencySpec

```rust
pub struct DependencySpec {
    pub runtime: String,
    pub dependencies: Vec<String>,
    pub requirements_file_content: Option<String>,
    pub min_version: Option<String>,
    pub max_version: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

### EnvironmentInfo

```rust
pub struct EnvironmentInfo {
    pub id: String,
    pub path: PathBuf,
    pub runtime: String,
    pub runtime_version: String,
    pub installed_dependencies: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_valid: bool,
    pub executable_path: PathBuf,
}
```

## Testing

Run dependency isolation tests:

```bash
cargo test --package attune-worker --test dependency_isolation_test
```

Key test scenarios:
- ✅ Virtual environment creation
- ✅ Idempotency (repeated ensure_environment calls)
- ✅ Dependency change detection
- ✅ Multi-pack isolation
- ✅ Environment validation
- ✅ Cleanup operations
- ✅ Pack reference sanitization

## References

- [Python venv documentation](https://docs.python.org/3/library/venv.html)
- [pip requirements files](https://pip.pypa.io/en/stable/reference/requirements-file-format/)
- [Semantic versioning](https://semver.org/)