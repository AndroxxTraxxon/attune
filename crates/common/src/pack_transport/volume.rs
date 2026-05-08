//! Volume-based pack transport (no-op — files already on shared volume).

use async_trait::async_trait;

use super::PackFileTransport;
use crate::error::Result;

/// No-op transport for when packs are on a shared volume.
#[derive(Debug)]
pub struct VolumePackTransport {
    packs_base_dir: String,
}

impl VolumePackTransport {
    pub fn new(packs_base_dir: &str) -> Self {
        Self {
            packs_base_dir: packs_base_dir.to_string(),
        }
    }
}

#[async_trait]
impl PackFileTransport for VolumePackTransport {
    async fn sync_pack(&self, pack_ref: &str) -> Result<()> {
        tracing::debug!(
            "VolumePackTransport: sync_pack('{}') — no-op (shared volume)",
            pack_ref
        );
        Ok(())
    }

    async fn remove_pack(&self, pack_ref: &str) -> Result<()> {
        tracing::debug!(
            "VolumePackTransport: remove_pack('{}') — no-op (shared volume)",
            pack_ref
        );
        Ok(())
    }

    async fn is_pack_local(&self, pack_ref: &str) -> bool {
        let pack_dir = std::path::Path::new(&self.packs_base_dir).join(pack_ref);
        pack_dir.is_dir()
    }

    fn transport_mode(&self) -> &'static str {
        "volume"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_volume_transport_is_pack_local() {
        let tmp = TempDir::new().unwrap();
        let transport = VolumePackTransport::new(tmp.path().to_str().unwrap());

        // Pack doesn't exist
        assert!(!transport.is_pack_local("mypack").await);

        // Create pack dir
        std::fs::create_dir(tmp.path().join("mypack")).unwrap();
        assert!(transport.is_pack_local("mypack").await);
    }

    #[tokio::test]
    async fn test_volume_transport_sync_is_noop() {
        let transport = VolumePackTransport::new("/tmp/packs");
        // Should not error
        transport.sync_pack("anything").await.unwrap();
        transport.remove_pack("anything").await.unwrap();
    }
}
