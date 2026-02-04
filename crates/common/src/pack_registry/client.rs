//! Registry client for fetching and parsing pack indices
//!
//! This module provides functionality for:
//! - Fetching index files from HTTP(S) and file:// URLs
//! - Caching indices with TTL-based expiration
//! - Searching packs across multiple registries
//! - Handling authenticated registries

use super::{PackIndex, PackIndexEntry};
use crate::config::{PackRegistryConfig, RegistryIndexConfig};
use crate::error::{Error, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

/// Cached registry index with expiration
#[derive(Clone)]
struct CachedIndex {
    /// The parsed index
    index: PackIndex,

    /// When this cache entry was created
    cached_at: SystemTime,

    /// TTL in seconds
    ttl: u64,
}

impl CachedIndex {
    /// Check if this cache entry is expired
    fn is_expired(&self) -> bool {
        match SystemTime::now().duration_since(self.cached_at) {
            Ok(duration) => duration.as_secs() > self.ttl,
            Err(_) => true, // If time went backwards, consider expired
        }
    }
}

/// Registry client for fetching and managing pack indices
pub struct RegistryClient {
    /// Configuration
    config: PackRegistryConfig,

    /// HTTP client
    http_client: reqwest::Client,

    /// Cache of fetched indices (URL -> CachedIndex)
    cache: Arc<RwLock<HashMap<String, CachedIndex>>>,
}

impl RegistryClient {
    /// Create a new registry client
    pub fn new(config: PackRegistryConfig) -> Result<Self> {
        let timeout = Duration::from_secs(config.timeout);

        let http_client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(format!("attune-registry-client/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| Error::Internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            http_client,
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get all enabled registries sorted by priority (lower number = higher priority)
    pub fn get_registries(&self) -> Vec<RegistryIndexConfig> {
        let mut registries: Vec<_> = self.config.indices
            .iter()
            .filter(|r| r.enabled)
            .cloned()
            .collect();

        // Sort by priority (ascending)
        registries.sort_by_key(|r| r.priority);

        registries
    }

    /// Fetch a pack index from a registry
    pub async fn fetch_index(&self, registry: &RegistryIndexConfig) -> Result<PackIndex> {
        // Check cache first if caching is enabled
        if self.config.cache_enabled {
            if let Some(cached) = self.get_cached_index(&registry.url) {
                if !cached.is_expired() {
                    tracing::debug!("Using cached index for registry: {}", registry.url);
                    return Ok(cached.index);
                }
            }
        }

        // Fetch fresh index
        tracing::info!("Fetching index from registry: {}", registry.url);
        let index = self.fetch_index_from_url(registry).await?;

        // Cache the result
        if self.config.cache_enabled {
            self.cache_index(&registry.url, index.clone());
        }

        Ok(index)
    }

    /// Fetch index from URL (bypassing cache)
    async fn fetch_index_from_url(&self, registry: &RegistryIndexConfig) -> Result<PackIndex> {
        let url = &registry.url;

        // Handle file:// URLs
        if url.starts_with("file://") {
            return self.fetch_index_from_file(url).await;
        }

        // Validate HTTPS if allow_http is false
        if !self.config.allow_http && url.starts_with("http://") {
            return Err(Error::Configuration(format!(
                "HTTP registry not allowed: {}. Set allow_http: true to enable.",
                url
            )));
        }

        // Build HTTP request
        let mut request = self.http_client.get(url);

        // Add custom headers
        for (key, value) in &registry.headers {
            request = request.header(key, value);
        }

        // Send request
        let response = request
            .send()
            .await
            .map_err(|e| Error::internal(format!("Failed to fetch registry index: {}", e)))?;

        // Check status
        if !response.status().is_success() {
            return Err(Error::internal(format!(
                "Registry returned error status {}: {}",
                response.status(),
                url
            )));
        }

        // Parse JSON
        let index: PackIndex = response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Failed to parse registry index: {}", e)))?;

        Ok(index)
    }

    /// Fetch index from file:// URL
    async fn fetch_index_from_file(&self, url: &str) -> Result<PackIndex> {
        let path = url.strip_prefix("file://")
            .ok_or_else(|| Error::Configuration(format!("Invalid file URL: {}", url)))?;

        let path = PathBuf::from(path);

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| Error::internal(format!("Failed to read index file: {}", e)))?;

        let index: PackIndex = serde_json::from_str(&content)
            .map_err(|e| Error::internal(format!("Failed to parse index file: {}", e)))?;

        Ok(index)
    }

    /// Get cached index if available
    fn get_cached_index(&self, url: &str) -> Option<CachedIndex> {
        let cache = self.cache.read().ok()?;
        cache.get(url).cloned()
    }

    /// Cache an index
    fn cache_index(&self, url: &str, index: PackIndex) {
        let cached = CachedIndex {
            index,
            cached_at: SystemTime::now(),
            ttl: self.config.cache_ttl,
        };

        if let Ok(mut cache) = self.cache.write() {
            cache.insert(url.to_string(), cached);
        }
    }

    /// Clear the cache
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    /// Search for a pack by reference across all registries
    pub async fn search_pack(&self, pack_ref: &str) -> Result<Option<(PackIndexEntry, String)>> {
        let registries = self.get_registries();

        for registry in registries {
            match self.fetch_index(&registry).await {
                Ok(index) => {
                    if let Some(pack) = index.packs.iter().find(|p| p.pack_ref == pack_ref) {
                        return Ok(Some((pack.clone(), registry.url.clone())));
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to fetch registry {}: {}",
                        registry.url,
                        e
                    );
                    continue;
                }
            }
        }

        Ok(None)
    }

    /// Search for packs by keyword across all registries
    pub async fn search_packs(&self, keyword: &str) -> Result<Vec<(PackIndexEntry, String)>> {
        let registries = self.get_registries();
        let mut results = Vec::new();
        let keyword_lower = keyword.to_lowercase();

        for registry in registries {
            match self.fetch_index(&registry).await {
                Ok(index) => {
                    for pack in index.packs {
                        // Search in ref, label, description, and keywords
                        let matches = pack.pack_ref.to_lowercase().contains(&keyword_lower)
                            || pack.label.to_lowercase().contains(&keyword_lower)
                            || pack.description.to_lowercase().contains(&keyword_lower)
                            || pack.keywords.iter().any(|k| k.to_lowercase().contains(&keyword_lower));

                        if matches {
                            results.push((pack, registry.url.clone()));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to fetch registry {}: {}",
                        registry.url,
                        e
                    );
                    continue;
                }
            }
        }

        Ok(results)
    }

    /// Get pack from specific registry
    pub async fn get_pack_from_registry(
        &self,
        pack_ref: &str,
        registry_name: &str,
    ) -> Result<Option<PackIndexEntry>> {
        // Find registry by name
        let registry = self.config.indices
            .iter()
            .find(|r| r.name.as_deref() == Some(registry_name))
            .ok_or_else(|| Error::not_found("registry", "name", registry_name))?;

        let index = self.fetch_index(registry).await?;

        Ok(index.packs.into_iter().find(|p| p.pack_ref == pack_ref))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RegistryIndexConfig;

    #[test]
    fn test_cached_index_expiration() {
        let index = PackIndex {
            registry_name: "Test".to_string(),
            registry_url: "https://example.com".to_string(),
            version: "1.0".to_string(),
            last_updated: "2024-01-20T12:00:00Z".to_string(),
            packs: vec![],
        };

        let cached = CachedIndex {
            index,
            cached_at: SystemTime::now(),
            ttl: 3600,
        };

        assert!(!cached.is_expired());

        // Test with expired cache
        let cached_old = CachedIndex {
            index: cached.index.clone(),
            cached_at: SystemTime::now() - Duration::from_secs(7200),
            ttl: 3600,
        };

        assert!(cached_old.is_expired());
    }

    #[test]
    fn test_get_registries_sorted() {
        let config = PackRegistryConfig {
            enabled: true,
            indices: vec![
                RegistryIndexConfig {
                    url: "https://registry3.example.com".to_string(),
                    priority: 3,
                    enabled: true,
                    name: Some("Registry 3".to_string()),
                    headers: HashMap::new(),
                },
                RegistryIndexConfig {
                    url: "https://registry1.example.com".to_string(),
                    priority: 1,
                    enabled: true,
                    name: Some("Registry 1".to_string()),
                    headers: HashMap::new(),
                },
                RegistryIndexConfig {
                    url: "https://registry2.example.com".to_string(),
                    priority: 2,
                    enabled: true,
                    name: Some("Registry 2".to_string()),
                    headers: HashMap::new(),
                },
                RegistryIndexConfig {
                    url: "https://disabled.example.com".to_string(),
                    priority: 0,
                    enabled: false,
                    name: Some("Disabled".to_string()),
                    headers: HashMap::new(),
                },
            ],
            cache_ttl: 3600,
            cache_enabled: true,
            timeout: 120,
            verify_checksums: true,
            allow_http: false,
        };

        let client = RegistryClient::new(config).unwrap();
        let registries = client.get_registries();

        assert_eq!(registries.len(), 3); // Disabled one excluded
        assert_eq!(registries[0].priority, 1);
        assert_eq!(registries[1].priority, 2);
        assert_eq!(registries[2].priority, 3);
    }
}
