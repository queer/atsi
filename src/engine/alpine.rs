use crate::util::{append_all, cache_dir, AtsiError, Result, SyncResult, USER_AGENT};

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use yaml_rust::{Yaml, YamlLoader};

pub const VERSION: &str = "3.14";
pub const ARCH: &str = "x86_64";

fn rootfs_base_directory() -> PathBuf {
    let mut path = cache_dir();
    path.push("alpine");
    path
}

pub fn rootfs_tarball(version: &str) -> PathBuf {
    let mut path = rootfs_base_directory();
    path.push(format!("alpine-rootfs-{}-{}.tar.gz", version, ARCH));
    path
}

pub fn rootfs_path(version: &str) -> PathBuf {
    let mut path = rootfs_base_directory();
    path.push(format!("alpine-rootfs-{}-{}", version, ARCH));
    path
}

pub async fn download_rootfs(version: &str) -> SyncResult<()> {
    if Path::new(&rootfs_tarball(version)).exists() {
        return Ok(());
    }
    info!("Downloading Alpine rootfs v{}...", version);
    let manifest_url = format!("{}/latest-releases.yaml", base_url(version, ARCH));
    let manifest_text = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?
        .get(manifest_url)
        .send()
        .await?
        .text()
        .await?;

    let docs = YamlLoader::load_from_str(manifest_text.as_str())?;
    let manifest = &docs[0];
    if let Some(vec) = manifest.as_vec() {
        let maybe_rootfs_manifest = vec.iter().find(|yaml| {
            matches!(
                yaml["flavor"].as_str(),
                Some("minirootfs") | Some("alpine-minirootfs")
            )
        });
        if let Some(rootfs_manifest) = maybe_rootfs_manifest {
            debug!("found alpine minirootfs! downloading...");
            download_rootfs_real(rootfs_manifest, version).await?;
            Ok(())
        } else {
            error!(
                "expected alpine minirootfs in manifest, but manifest was\n{}",
                manifest_text
            );
            Err(Box::new(AtsiError::AlpineManifestMissing))
        }
    } else {
        Err(Box::new(AtsiError::AlpineManifestInvalid))
    }
}

async fn download_rootfs_real(rootfs_manifest: &Yaml, version: &str) -> SyncResult<PathBuf> {
    match rootfs_manifest["file"].as_str() {
        Some(rootfs_filename) => {
            // minirootfs is a ~3MB tarball, so we can afford to hold
            // it all in memory.
            let rootfs_url = format!("{}/{}", base_url(version, ARCH), rootfs_filename);

            let download_response = reqwest::get(rootfs_url).await?;
            let rootfs_bytes = download_response.bytes().await?;

            let output_path = rootfs_tarball(version);
            fs::create_dir_all(&rootfs_base_directory())?;
            let mut output_file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output_path)?;
            output_file.write_all(&rootfs_bytes)?;
            Ok(output_path)
        }
        None => Err(Box::new(AtsiError::AlpineManifestFileMissing)),
    }
}

pub fn extract_rootfs_to_path(version: &str, target: &Path) -> Result<()> {
    extract_tarball(&rootfs_tarball(version), target)?;
    setup_rootfs(target)?;
    Ok(())
}

fn extract_tarball(path: &PathBuf, target_path: &Path) -> Result<()> {
    let tarball = fs::File::open(path)?;
    let tar = flate2::read::GzDecoder::new(tarball);
    let mut archive = tar::Archive::new(tar);
    archive.unpack(target_path)?;
    Ok(())
}

fn setup_rootfs(rootfs: &Path) -> Result<()> {
    File::create(append_all(rootfs, vec!["dev", "null"]))?;
    File::create(append_all(rootfs, vec!["dev", "zero"]))?;
    File::create(append_all(rootfs, vec!["dev", "random"]))?;
    File::create(append_all(rootfs, vec!["dev", "urandom"]))?;
    File::create(append_all(rootfs, vec!["dev", "console"]))?;
    fs::create_dir_all(append_all(rootfs, vec!["dev", "shm"]))?;
    fs::create_dir_all(append_all(rootfs, vec!["dev", "pts"]))?;
    fs::create_dir_all(append_all(rootfs, vec!["proc"]))?;
    fs::create_dir_all(append_all(rootfs, vec!["sys"]))?;
    fs::create_dir_all(append_all(rootfs, vec!["app"]))?;

    // networking
    let mut resolv = File::create(append_all(rootfs, vec!["etc", "resolv.conf"]))?;
    resolv.write_all("nameserver 10.0.2.3".as_bytes())?; // slirp4netns
    Ok(())
}

fn base_url(version: &str, arch: &str) -> String {
    format!(
        "https://cz.alpinelinux.org/alpine/v{}/releases/{}",
        version, arch
    )
}
