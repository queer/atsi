use std::error::Error;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;
pub type SyncResult<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

pub const USER_AGENT: &str = "squish (https://github.com/queer/squish)";

#[allow(dead_code)]
#[derive(Debug)]
pub enum AtsiError {
    GenericError(Box<dyn std::error::Error + Send + Sync>),

    SlirpSocketCouldntBeFound,

    AlpineManifestInvalid,
    AlpineManifestMissing,
    AlpineManifestFileMissing,
}

impl std::fmt::Display for AtsiError {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        todo!()
    }
}

impl Error for AtsiError {}

pub fn append_all(buf: &Path, parts: Vec<&str>) -> PathBuf {
    let mut buf = buf.to_path_buf();
    for part in parts {
        buf.push(part);
    }
    buf
}

pub fn cache_dir() -> PathBuf {
    let mut path = dirs::cache_dir().unwrap();
    path.push("@");
    path
}
