use crate::util::{append_all, Result, SyncResult};

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use nix::mount::{mount, MsFlags};

pub struct FsDriver;

impl FsDriver {
    pub fn new() -> Self {
        Self {}
    }

    pub fn all_containers_root(&self) -> PathBuf {
        let data_dir = dirs::data_dir().expect("Must have data dir to run atsi containers");
        append_all(&data_dir, vec!["@", "containers"])
    }

    pub fn container_root(&self, name: &str) -> PathBuf {
        append_all(&self.all_containers_root(), vec![name])
    }

    pub fn persistence_file(&self, name: &str) -> PathBuf {
        append_all(&self.container_root(name), vec!["state.json"])
    }

    pub fn cleanup_root(&self, name: &str) -> SyncResult<()> {
        fs::remove_dir_all(self.container_root(name))?;
        Ok(())
    }

    pub fn bind_mount_dev(&self, dev: &'static str, target: &Path) -> Result<()> {
        mount(Some(dev), target, Some(""), MsFlags::MS_BIND, Some(""))?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn bind_mount_ro(&self, src: &Path, target: &Path) -> Result<()> {
        // ro bindmount is a complicated procedure: https://unix.stackexchange.com/a/128388
        // tldr: You first do a normal bindmount, then remount bind+ro
        self.bind_mount(src, target, MsFlags::MS_NOATIME | MsFlags::MS_NODIRATIME)?;
        self.remount_ro(target)?;
        Ok(())
    }

    pub fn remount_ro(&self, target: &Path) -> Result<()> {
        mount::<Path, Path, str, str>(
            None,
            target,
            Some(""),
            MsFlags::MS_REMOUNT | MsFlags::MS_BIND | MsFlags::MS_RDONLY,
            Some(""),
        )?;
        Ok(())
    }

    pub fn bind_mount_rw(&self, src: &Path, target: &Path) -> Result<()> {
        self.bind_mount(src, target, MsFlags::MS_BIND)
    }

    fn bind_mount(&self, src: &Path, target: &Path, flags: MsFlags) -> Result<()> {
        debug!("bind-mount {} -> {}", src.display(), target.display());
        mount(
            Some(src),
            target,
            Some(""),
            MsFlags::MS_BIND | flags,
            Some(""),
        )?;
        Ok(())
    }

    pub fn touch(&self, path: &Path) -> Result<()> {
        debug!("touching: {}",  path.display());
        match OpenOptions::new().create(true).write(true).open(path) {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }

    pub fn touch_dir(&self, path: &Path) -> Result<()> {
        debug!("touching dir: {}",  path.display());
        match fs::create_dir_all(path) {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }

    pub fn touch_dir_sync(&self, path: &Path) -> SyncResult<()> {
        debug!("touching dir: {}",  path.display());
        match fs::create_dir_all(path) {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }
}
