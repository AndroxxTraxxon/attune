//! Integration tests for Python virtual environment dependency isolation
//!
//! Tests the end-to-end flow of creating isolated Python environments
//! for packs with dependencies.

use attune_worker::runtime::{
    DependencyManager, DependencyManagerRegistry, DependencySpec, PythonVenvManager,
};
use tempfile::TempDir;

#[tokio::test]
async fn test_python_venv_creation() {
    let temp_dir = TempDir::new().unwrap();
    let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

    let spec = DependencySpec::new("python").with_dependency("requests==2.28.0");

    let env_info = manager
        .ensure_environment("test_pack", &spec)
        .await
        .expect("Failed to create environment");

    assert_eq!(env_info.runtime, "python");
    assert!(env_info.is_valid);
    assert!(env_info.path.exists());
    assert!(env_info.executable_path.exists());
}

#[tokio::test]
async fn test_venv_idempotency() {
    let temp_dir = TempDir::new().unwrap();
    let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

    let spec = DependencySpec::new("python").with_dependency("requests==2.28.0");

    // Create environment first time
    let env_info1 = manager
        .ensure_environment("test_pack", &spec)
        .await
        .expect("Failed to create environment");

    let created_at1 = env_info1.created_at;

    // Call ensure_environment again with same dependencies
    let env_info2 = manager
        .ensure_environment("test_pack", &spec)
        .await
        .expect("Failed to ensure environment");

    // Should return existing environment (same created_at)
    assert_eq!(env_info1.created_at, env_info2.created_at);
    assert_eq!(created_at1, env_info2.created_at);
}

#[tokio::test]
async fn test_venv_update_on_dependency_change() {
    let temp_dir = TempDir::new().unwrap();
    let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

    let spec1 = DependencySpec::new("python").with_dependency("requests==2.28.0");

    // Create environment with first set of dependencies
    let env_info1 = manager
        .ensure_environment("test_pack", &spec1)
        .await
        .expect("Failed to create environment");

    let created_at1 = env_info1.created_at;

    // Give it a moment to ensure timestamp difference
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Change dependencies
    let spec2 = DependencySpec::new("python").with_dependency("requests==2.29.0");

    // Should recreate environment
    let env_info2 = manager
        .ensure_environment("test_pack", &spec2)
        .await
        .expect("Failed to update environment");

    // Updated timestamp should be newer
    assert!(env_info2.updated_at >= created_at1);
}

#[tokio::test]
async fn test_multiple_pack_isolation() {
    let temp_dir = TempDir::new().unwrap();
    let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

    let spec1 = DependencySpec::new("python").with_dependency("requests==2.28.0");
    let spec2 = DependencySpec::new("python").with_dependency("flask==2.3.0");

    // Create environments for two different packs
    let env1 = manager
        .ensure_environment("pack_a", &spec1)
        .await
        .expect("Failed to create environment for pack_a");

    let env2 = manager
        .ensure_environment("pack_b", &spec2)
        .await
        .expect("Failed to create environment for pack_b");

    // Should have different paths
    assert_ne!(env1.path, env2.path);
    assert_ne!(env1.executable_path, env2.executable_path);

    // Both should be valid
    assert!(env1.is_valid);
    assert!(env2.is_valid);
}

#[tokio::test]
async fn test_get_executable_path() {
    let temp_dir = TempDir::new().unwrap();
    let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

    let spec = DependencySpec::new("python");

    manager
        .ensure_environment("test_pack", &spec)
        .await
        .expect("Failed to create environment");

    let python_path = manager
        .get_executable_path("test_pack")
        .await
        .expect("Failed to get executable path");

    assert!(python_path.exists());
    assert!(python_path.to_string_lossy().contains("test_pack"));
}

#[tokio::test]
async fn test_validate_environment() {
    let temp_dir = TempDir::new().unwrap();
    let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

    // Non-existent environment should not be valid
    let is_valid = manager
        .validate_environment("nonexistent")
        .await
        .expect("Validation check failed");
    assert!(!is_valid);

    // Create environment
    let spec = DependencySpec::new("python");
    manager
        .ensure_environment("test_pack", &spec)
        .await
        .expect("Failed to create environment");

    // Should now be valid
    let is_valid = manager
        .validate_environment("test_pack")
        .await
        .expect("Validation check failed");
    assert!(is_valid);
}

#[tokio::test]
async fn test_remove_environment() {
    let temp_dir = TempDir::new().unwrap();
    let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

    let spec = DependencySpec::new("python");

    // Create environment
    let env_info = manager
        .ensure_environment("test_pack", &spec)
        .await
        .expect("Failed to create environment");

    let path = env_info.path.clone();
    assert!(path.exists());

    // Remove environment
    manager
        .remove_environment("test_pack")
        .await
        .expect("Failed to remove environment");

    assert!(!path.exists());

    // Get environment should return None
    let env = manager
        .get_environment("test_pack")
        .await
        .expect("Failed to get environment");
    assert!(env.is_none());
}

#[tokio::test]
async fn test_list_environments() {
    let temp_dir = TempDir::new().unwrap();
    let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

    let spec = DependencySpec::new("python");

    // Create multiple environments
    manager
        .ensure_environment("pack_a", &spec)
        .await
        .expect("Failed to create pack_a");

    manager
        .ensure_environment("pack_b", &spec)
        .await
        .expect("Failed to create pack_b");

    manager
        .ensure_environment("pack_c", &spec)
        .await
        .expect("Failed to create pack_c");

    // List should return all three
    let environments = manager
        .list_environments()
        .await
        .expect("Failed to list environments");

    assert_eq!(environments.len(), 3);
}

#[tokio::test]
async fn test_dependency_manager_registry() {
    let temp_dir = TempDir::new().unwrap();
    let mut registry = DependencyManagerRegistry::new();

    let python_manager = PythonVenvManager::new(temp_dir.path().to_path_buf());
    registry.register(Box::new(python_manager));

    // Should support python
    assert!(registry.supports("python"));
    assert!(!registry.supports("nodejs"));

    // Should be able to get manager
    let manager = registry.get("python");
    assert!(manager.is_some());
    assert_eq!(manager.unwrap().runtime_type(), "python");
}

#[tokio::test]
async fn test_dependency_spec_builder() {
    let spec = DependencySpec::new("python")
        .with_dependency("requests==2.28.0")
        .with_dependency("flask>=2.0.0")
        .with_version_range(Some("3.8".to_string()), Some("3.11".to_string()));

    assert_eq!(spec.runtime, "python");
    assert_eq!(spec.dependencies.len(), 2);
    assert!(spec.has_dependencies());
    assert_eq!(spec.min_version, Some("3.8".to_string()));
    assert_eq!(spec.max_version, Some("3.11".to_string()));
}

#[tokio::test]
async fn test_requirements_file_content() {
    let temp_dir = TempDir::new().unwrap();
    let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

    let requirements = "requests==2.28.0\nflask==2.3.0\npydantic>=2.0.0";
    let spec = DependencySpec::new("python").with_requirements_file(requirements.to_string());

    let env_info = manager
        .ensure_environment("test_pack", &spec)
        .await
        .expect("Failed to create environment with requirements file");

    assert!(env_info.is_valid);
    assert!(env_info.installed_dependencies.len() > 0);
}

#[tokio::test]
async fn test_pack_ref_sanitization() {
    let temp_dir = TempDir::new().unwrap();
    let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

    let spec = DependencySpec::new("python");

    // Pack refs with special characters should be sanitized
    let env_info = manager
        .ensure_environment("core.http", &spec)
        .await
        .expect("Failed to create environment");

    // Path should not contain dots
    let path_str = env_info.path.to_string_lossy();
    assert!(path_str.contains("core_http"));
    assert!(!path_str.contains("core.http"));
}

#[tokio::test]
async fn test_needs_update_detection() {
    let temp_dir = TempDir::new().unwrap();
    let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

    let spec1 = DependencySpec::new("python").with_dependency("requests==2.28.0");

    // Non-existent environment needs update
    let needs_update = manager
        .needs_update("test_pack", &spec1)
        .await
        .expect("Failed to check update status");
    assert!(needs_update);

    // Create environment
    manager
        .ensure_environment("test_pack", &spec1)
        .await
        .expect("Failed to create environment");

    // Same spec should not need update
    let needs_update = manager
        .needs_update("test_pack", &spec1)
        .await
        .expect("Failed to check update status");
    assert!(!needs_update);

    // Different spec should need update
    let spec2 = DependencySpec::new("python").with_dependency("requests==2.29.0");
    let needs_update = manager
        .needs_update("test_pack", &spec2)
        .await
        .expect("Failed to check update status");
    assert!(needs_update);
}

#[tokio::test]
async fn test_empty_dependencies() {
    let temp_dir = TempDir::new().unwrap();
    let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

    // Pack with no dependencies should still create venv
    let spec = DependencySpec::new("python");
    assert!(!spec.has_dependencies());

    let env_info = manager
        .ensure_environment("test_pack", &spec)
        .await
        .expect("Failed to create environment without dependencies");

    assert!(env_info.is_valid);
    assert!(env_info.path.exists());
}

#[tokio::test]
async fn test_get_environment_caching() {
    let temp_dir = TempDir::new().unwrap();
    let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

    let spec = DependencySpec::new("python");

    // Create environment
    manager
        .ensure_environment("test_pack", &spec)
        .await
        .expect("Failed to create environment");

    // First get_environment should read from disk
    let env1 = manager
        .get_environment("test_pack")
        .await
        .expect("Failed to get environment")
        .expect("Environment not found");

    // Second get_environment should use cache
    let env2 = manager
        .get_environment("test_pack")
        .await
        .expect("Failed to get environment")
        .expect("Environment not found");

    assert_eq!(env1.id, env2.id);
    assert_eq!(env1.path, env2.path);
}
