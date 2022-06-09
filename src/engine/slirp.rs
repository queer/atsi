use crate::util::{append_all, cache_dir, AtsiError, SyncResult, USER_AGENT};

use std::fs;
use std::fs::Permissions;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::prelude::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::time::sleep;

const URL: &str = "https://github.com/rootless-containers/slirp4netns/releases/download/v1.2.0/slirp4netns-x86_64";

pub fn bin_path() -> PathBuf {
    append_all(&cache_dir(), vec!["slirp4netns"])
}

pub async fn download_slirp4netns() -> SyncResult<()> {
    let output_path = &bin_path();

    if Path::new(output_path).exists() {
        return Ok(());
    }

    debug!("downloading slirp4netns binary from {}", URL);
    let slirp_bytes = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?
        .get(URL)
        .send()
        .await?
        .bytes()
        .await?;
    let mut output_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)?;
    output_file.write_all(&slirp_bytes)?;
    fs::set_permissions(output_path, Permissions::from_mode(0o755))?;
    // eprintln!("{:o}", output_file.metadata()?.permissions().mode());
    Ok(())
}

fn slirp_socket_path(name: &str) -> String {
    format!("/tmp/slirp4netns-{}.sock", name)
}

pub async fn spawn_for_container(name: &str, pid: u32) -> SyncResult<tokio::process::Child> {
    let slirp_socket_path = slirp_socket_path(name);
    let slirp = tokio::process::Command::new(&bin_path())
        .args(vec![
            "--configure",
            "--mtu=65520",
            "--disable-host-loopback",
            "--api-socket",
            slirp_socket_path.as_str(),
            format!("{}", pid).as_str(),
            "tap0",
        ])
        // TODO: Should we be capturing these logs?
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(slirp)
}

pub async fn add_port_forward(name: &str, host: &u16, container: &u16) -> SyncResult<String> {
    slirp_exec(
        &slirp_socket_path(name),
        format!(
            r#"
        {{
            "execute": "add_hostfwd",
            "arguments": {{
                "proto": "tcp",
                "host_ip": "127.0.0.1",
                "host_port": {},
                "guest_port": {}
            }}
        }}
    "#,
            host, container
        )
        .as_str(),
    )
    .await
}

async fn slirp_exec(slirp_socket_path: &str, command: &str) -> SyncResult<String> {
    debug!("connecting to: {}", slirp_socket_path);
    let mut attempts: u8 = 0;
    let mut slirp_socket;
    loop {
        if let Ok(s) = UnixStream::connect(slirp_socket_path) {
            slirp_socket = s;
            break;
        }
        attempts += 1;
        if attempts > 100 {
            return Err(Box::new(AtsiError::SlirpSocketCouldntBeFound));
        }
        sleep(Duration::from_millis(1)).await;
    }
    debug!("slirp socket connected (attempts={})", attempts);
    slirp_socket.write_all(command.as_bytes())?;
    let mut res = String::new();
    slirp_socket.read_to_string(&mut res)?;
    Ok(res)
}
