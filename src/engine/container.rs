use std::fs::File;
use std::io::Write;
use std::os::unix::prelude::CommandExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::fs_driver::FsDriver;
use crate::util::{append_all, Result, SyncResult};

use nix::sched::{clone, CloneFlags};
use nix::sys::wait::{waitpid, WaitStatus};
use rlimit::Resource;
use tokio::time::Instant;

pub struct ContainerEngine<'a> {
    name: &'a str,
    fs: FsDriver,
    opts: super::RunOpts,
}

#[derive(serde::Serialize, serde::Deserialize, derive_getters::Getters)]
pub struct PersistentState {
    name: String,
    command: String,
    detach: bool,
    pid: u32,
    slirp_pid: u32,
}

impl<'a> ContainerEngine<'a> {
    pub fn new(name: &'a str, opts: super::RunOpts) -> Self {
        Self {
            name,
            fs: FsDriver::new(),
            opts,
        }
    }

    pub async fn run(&mut self, start: Instant) -> SyncResult<()> {
        // Basic setup
        self.fs.touch_dir_sync(&self.fs.container_root(self.name))?;

        // clone(2)
        let stack_size = match Resource::STACK.get() {
            Ok((soft, _hard)) => soft as usize,
            Err(_) => {
                // 8MB
                8 * 1024 * 1024
            }
        };

        let callback = || match self.run_in_container(start) {
            Ok(_) => 0,
            Err(err) => {
                error!("uncaught error! {}", err);
                1
            }
        };

        let mut stack_vec = vec![0u8; stack_size];
        let stack: &mut [u8] = stack_vec.as_mut_slice();

        let pid = clone(
            Box::new(callback),
            stack,
            CloneFlags::CLONE_NEWPID
                | CloneFlags::CLONE_NEWUTS
                | CloneFlags::CLONE_NEWNS
                | CloneFlags::CLONE_NEWNET
                | CloneFlags::CLONE_NEWUSER
                | CloneFlags::CLONE_NEWCGROUP,
            Some(nix::sys::signal::Signal::SIGCHLD as i32),
        )?;
        if (pid.as_raw() as i32) == -1 {
            error!("clone error");
            error!("{:?}", std::io::Error::last_os_error());
            return Err(Box::new(std::io::Error::last_os_error()));
        }

        // slirp4netns
        let mut slirp = super::slirp::spawn_for_container(self.name, pid.as_raw() as u32).await?;
        self.persist(slirp.id().unwrap())?;
        let ports = self.opts.ports.clone();
        let name = self.name.to_string();
        let slirp_id = slirp.id().unwrap();
        tokio::spawn(async move {
            for (outer, inner) in ports {
                super::slirp::add_port_forward(&name, &outer, &inner)
                    .await
                    .unwrap();
            }
            slirp.wait().await.unwrap();
        });

        let name = self.name.to_string();
        #[allow(unused_must_use)]
        ctrlc::set_handler(move || {
            debug!("Cleaning up after ^C");
            // It's okay to ignore the result here because we don't actually
            // care. This is just a fail-safe on the off-chance that it doesn't
            // otherwise get cleaned up.
            // We do need to have this, because otherwise the normal cleanup
            // routine may not be called.
            FsDriver::new().cleanup_root(name.as_str());
            nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(slirp_id as i32),
                nix::sys::signal::SIGTERM,
            )
            .unwrap();
        })?;

        // wait for exit
        loop {
            match waitpid(pid, None) {
                Ok(WaitStatus::Exited(_pid, _status)) => {
                    break;
                }
                Err(nix::errno::Errno::ECHILD) => {
                    // We might need to wait to let stdout/err buffer
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    break;
                }
                _ => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }

        debug!("Cleaning up!");
        self.fs.cleanup_root(self.name)?;
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(slirp_id as i32),
            nix::sys::signal::SIGTERM,
        )?;

        Ok(())
    }

    fn persist(&self, slirp_pid: u32) -> SyncResult<()> {
        debug!(
            "Persist state -> {}",
            self.fs.persistence_file(self.name).display()
        );
        let state = PersistentState {
            name: self.name.to_string(),
            command: self.opts.command.to_string(),
            detach: self.opts.detach,
            pid: std::process::id(),
            slirp_pid,
        };
        let ser = serde_json::to_string(&state)?;
        let mut file = File::create(self.fs.persistence_file(self.name))?;
        file.write_all(ser.as_bytes())?;
        Ok(())
    }

    fn run_in_container(&mut self, start: Instant) -> Result<()> {
        use nix::unistd::{chdir, chroot};

        let container_root = &self.fs.container_root(self.name);
        let rootfs_lower = &append_all(container_root, vec!["rootfs_lower"]);
        let rootfs = &append_all(container_root, vec!["rootfs"]);

        // Set up root directory and bind-mount immutable alpine fs
        debug!("Setting up root directory...");
        self.fs.touch_dir(rootfs_lower)?;
        self.fs.touch_dir(rootfs)?;
        super::alpine::extract_rootfs_to_path(&self.opts.alpine_version, rootfs_lower)?;
        self.fs.bind_mount_rw(rootfs_lower, rootfs)?;

        // Mount basic devices
        debug!("Bind-mounting devices...");
        self.fs
            .bind_mount_dev("/dev/null", &append_all(rootfs, vec!["dev", "null"]))?;
        self.fs
            .bind_mount_dev("/dev/zero", &append_all(rootfs, vec!["dev", "zero"]))?;
        self.fs
            .bind_mount_dev("/dev/random", &append_all(rootfs, vec!["dev", "random"]))?;
        self.fs
            .bind_mount_dev("/dev/urandom", &append_all(rootfs, vec!["dev", "urandom"]))?;

        // Make a fake /tmp and mount it rw
        debug!("Mounting /tmp...");
        let tmpfs = &append_all(container_root, vec!["tmp"]);
        self.fs.touch_dir(tmpfs)?;
        self.fs
            .bind_mount_rw(tmpfs, &append_all(rootfs, vec!["tmp"]))?;

        // User mounts
        debug!("mounting user rw mounts...");
        self.auto_mount(rootfs, &self.opts.rw_mounts, AutoMountMode::Rw)?;
        debug!("mounting user ro mounts...");
        self.auto_mount(rootfs, &self.opts.ro_mounts, AutoMountMode::Ro)?;

        // chroot
        debug!("Pivoting!");
        debug!("pivotroot -> {}", rootfs.display());
        chroot(rootfs).expect("couldn't chroot!?");
        chdir("/app").expect("couldn't chdir to /app!?");

        debug!("container started in: {:?}", start.elapsed());

        use std::process::Command;

        if !self.opts.packages.is_empty() {
            info!("installing {} package(s)...", self.opts.packages.len());
            info!("requested packages: {}", self.opts.packages.join(", "));
            let _update_status = Command::new("/sbin/apk")
                .env_clear()
                .arg("update")
                .spawn()?
                .wait()?;
            let mut install_args = vec!["add"];
            for pkg in &self.opts.packages {
                install_args.push(pkg);
            }
            let _install_status = Command::new("/sbin/apk")
                .env_clear()
                .args(install_args)
                .spawn()?
                .wait()?;
            info!("installed!");
        }

        if self.opts.immutable {
            info!("making container immutable!");
            debug!("remounting rootfs as ro!");
            self.fs.remount_ro(&PathBuf::from("/"))?;
            debug!("rootfs remounted ro!");
        }

        // This will never return if the container successfully starts
        let error = Command::new("sh")
            .env_clear()
            .arg("-c")
            .arg(&self.opts.command)
            .exec();
        error!("Failed running container: {}", error);

        Ok(())
    }

    fn auto_mount(
        &self,
        rootfs: &Path,
        mounts: &Vec<(String, String)>,
        mode: AutoMountMode,
    ) -> Result<()> {
        for (source, target) in mounts {
            let source = Path::new(&source);
            let target_path = format!("{}/{}", rootfs.display(), target);
            let target = Path::new(&target_path);
            debug!("mounting {:?}: {} -> {}", mode, source.display(), target.display());
            if source.metadata()?.is_dir() {
                self.fs.touch_dir(Path::new(&target))?;
                match &mode {
                    AutoMountMode::Rw => {
                        self.fs
                            .bind_mount_rw(&Path::new(source).canonicalize()?, target)?;
                    }
                    AutoMountMode::Ro => {
                        self.fs
                            .bind_mount_ro(&Path::new(source).canonicalize()?, target)?;
                    }
                }
            } else {
                self.fs.touch_dir(Path::new(&target).parent().unwrap())?;
                self.fs.touch(Path::new(&target))?;
                match &mode {
                    AutoMountMode::Rw => {
                        self.fs
                            .bind_mount_rw(&Path::new(source).canonicalize()?, target)?;
                    }
                    AutoMountMode::Ro => {
                        self.fs
                            .bind_mount_ro(&Path::new(source).canonicalize()?, target)?;
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
enum AutoMountMode {
    Rw,
    Ro,
}
