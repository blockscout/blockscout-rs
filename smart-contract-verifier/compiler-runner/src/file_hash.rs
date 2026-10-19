// SPDX-License-Identifier: LicenseRef-Blockscout

use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

/// Identity of a local file. `ctime` changes on every content or metadata change and cannot be set
/// from user space, so an unchanged fingerprint means unchanged bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileFingerprint {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}

impl FileFingerprint {
    pub fn new(metadata: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
            len: metadata.len(),
            mtime: (metadata.mtime(), metadata.mtime_nsec()),
            ctime: (metadata.ctime(), metadata.ctime_nsec()),
        }
    }
}

/// SHA-256 of the file at `path`, which must still be the file `expected` was taken from and must
/// not change while it is read.
pub fn sha256_file(path: &Path, expected: FileFingerprint) -> std::io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let hashed = std::io::copy(&mut (&mut file).take(expected.len), &mut hasher)?;
    if hashed != expected.len || FileFingerprint::new(&file.metadata()?) != expected {
        return Err(std::io::Error::other(format!(
            "{} changed while it was being hashed",
            path.display()
        )));
    }
    Ok(hasher.finalize().into())
}
