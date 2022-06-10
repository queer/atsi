pub mod alpine;
pub mod container;
pub mod fs_driver;
pub mod slirp;

use tokio::time::Instant;

use crate::util::SyncResult;

use std::fs;
use std::path::Path;

pub struct RunOpts {
    pub command: String,
    pub packages: Vec<String>,
    pub detach: bool,
    pub ports: Vec<(u16, u16)>,
    pub immutable: bool,
    pub rw_mounts: Vec<(String, String)>,
    pub ro_mounts: Vec<(String, String)>,
    pub alpine_version: String,
}

pub struct Engine {
    name: String,
    start: Instant,
}

impl Engine {
    pub fn new(start: Instant) -> Self {
        Self {
            name: haikunator::Haikunator::default().haikunate(),
            start,
        }
    }

    pub async fn run(
        &self,
        opts: RunOpts,
    ) -> SyncResult<()> {
        container::ContainerEngine::new(&self.name, opts)
            .run(self.start)
            .await?;
        Ok(())
    }

    pub async fn ps(&self) -> SyncResult<()> {
        let driver = fs_driver::FsDriver::new();
        let mut dead_containers = vec![];
        let mut live_containers = vec![];
        for container in fs::read_dir(driver.all_containers_root())? {
            let state = fs::read_to_string(
                driver.persistence_file(&container?.file_name().to_string_lossy().to_string()),
            )?;
            let state: container::PersistentState = serde_json::from_str(&state)?;
            // Check if pid is still alive
            // - if alive, add to live queue
            // - if dead, add to purge queue
            let path = format!("/proc/{}", state.pid());
            let path = Path::new(&path);
            if path.exists() {
                live_containers.push(state);
            } else {
                dead_containers.push(state);
            }
        }

        for container in dead_containers {
            let root = driver.container_root(container.name());
            fs::remove_dir_all(root)?;
            warn!("Purged dead container {}", container.name());
        }

        for container in live_containers {
            info!("{}: {}", container.name(), container.pid());
        }

        Ok(())
    }
}
