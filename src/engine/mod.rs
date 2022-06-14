pub mod alpine;
pub mod container;
pub mod fs_driver;
pub mod slirp;

use tokio::time::Instant;

use crate::util::{cache_dir, SyncResult};

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use self::fs_driver::FsDriver;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct RunOpts {
    pub command: String,
    pub name: String,
    pub packages: Vec<String>,
    pub detach: bool,
    pub ports: Vec<(u16, u16)>,
    pub immutable: bool,
    pub rw_mounts: Vec<(String, String)>,
    pub ro_mounts: Vec<(String, String)>,
    pub alpine_version: String,
    pub env_vars: HashMap<String, String>,
}

pub struct Engine {
    start: Instant,
    fs: FsDriver,
}

impl Engine {
    pub fn new(start: Instant) -> Self {
        Self {
            start,
            fs: FsDriver::new(),
        }
    }

    pub async fn init(&self) -> SyncResult<()> {
        tokio::fs::create_dir_all(&self.fs.all_containers_root()).await?;
        tokio::fs::create_dir_all(&cache_dir()).await?;
        Ok(())
    }

    pub async fn run(&self, opts: RunOpts) -> SyncResult<()> {
        container::ContainerEngine::new(opts)
            .run(self.start)
            .await?;
        Ok(())
    }

    pub fn container_exists(&self, name: &str) -> bool {
        self.fs.container_root(name).exists()
    }

    pub async fn ps(&self, json: bool) -> SyncResult<()> {
        use prettytable::{cell, row, Table};

        let mut dead_containers = vec![];
        let mut live_containers = vec![];
        for container in fs::read_dir(self.fs.all_containers_root())? {
            let state = fs::read_to_string(
                self.fs
                    .persistence_file(&container?.file_name().to_string_lossy().to_string()),
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
            let root = self.fs.container_root(container.name());
            fs::remove_dir_all(root)?;
            nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(*container.pid() as i32),
                nix::sys::signal::SIGTERM,
            )?;
            nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(*container.slirp_pid() as i32),
                nix::sys::signal::SIGTERM,
            )?;
            warn!("purged dead container {}", container.name());
        }

        if json {
            println!("{}", serde_json::to_string(&live_containers)?);
        } else {
            let mut table = Table::new();
            table.add_row(row!["NAME", "PID", "COMMAND"]);
            for container in live_containers {
                table.add_row(row![
                    container.name(),
                    container.pid(),
                    container.opts().command
                ]);
            }
            table.printstd();
        }

        Ok(())
    }
}
