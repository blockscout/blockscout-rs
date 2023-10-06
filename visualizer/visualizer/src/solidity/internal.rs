use crate::metrics;
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;
use tokio::process::Command;

#[derive(Debug, Error)]
pub enum Error {
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
    #[error("sol2uml call failed: {0}")]
    Sol2Uml(String),
    #[error("failed to save files")]
    SaveFiles(#[from] std::io::Error),
}

pub async fn save_files(root: &Path, files: BTreeMap<PathBuf, String>) -> Result<(), Error> {
    let join = files.into_iter().map(|(name, content)| {
        let root = root.to_owned();
        tokio::task::spawn_blocking(move || -> Result<(), std::io::Error> {
            if name.has_root() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Error. All paths should be relative.",
                ));
            }

            // Set a default file prefix if none is provided
            let name = if name.ends_with(".sol") {
                name.with_file_name("main.sol")
            } else {
                name
            };

            let file_path = root.join(name);
            let prefix = file_path.parent();
            if let Some(prefix) = prefix {
                std::fs::create_dir_all(prefix)?;
            }
            let mut f = std::fs::File::create(file_path)?;
            f.write_all(content.as_bytes())?;
            Ok(())
        })
    });
    let results: Vec<_> = futures::future::join_all(join).await;

    for result in results {
        result.map_err(anyhow::Error::msg)??;
    }

    Ok(())
}

pub struct Sol2Uml {
    command: Command,
}

#[allow(unused)]
impl Sol2Uml {
    pub fn new() -> Self {
        Self {
            command: Command::new("sol2uml"),
        }
    }

    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.command.arg(arg);
        self
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.command.args(args);
        self
    }

    pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
        self.command.current_dir(dir);
        self
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn call(&mut self) -> Result<(), Error> {
        let output = {
            let _timer = metrics::SOL2UML_EXECUTION_TIME.start_timer();
            self.command.output().await.map_err(anyhow::Error::msg)?
        };

        tracing::info!(output = ?output, "process finished");

        if output.status.success() && output.stderr.is_empty() {
            Ok(())
        } else {
            Err(Error::Sol2Uml(
                std::str::from_utf8(&output.stderr)
                    .map_err(anyhow::Error::msg)?
                    .to_owned(),
            ))
        }
    }
}
