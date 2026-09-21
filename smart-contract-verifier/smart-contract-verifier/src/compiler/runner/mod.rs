// SPDX-License-Identifier: LicenseRef-Blockscout

mod concurrency;
mod docker;
mod metrics;
mod native;

use async_trait::async_trait;
use bytes::Bytes;
use std::{
    collections::{HashMap, HashSet},
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    sync::OwnedSemaphorePermit,
};

pub use concurrency::ConcurrencyLimitedCompilerExecutor;
pub use docker::{DockerCompilerExecutor, DockerCompilerExecutorSettings};
pub use native::NativeCompilerExecutor;

pub const DEFAULT_EXECUTION_TIMEOUT_SECS: u64 = 600;
const DEFAULT_MAX_UPLOAD_BYTES: u64 = 256 * 1024 * 1024;
pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 256 * 1024 * 1024;

async fn deliver_stdin<W>(
    writer: &mut W,
    input: &[u8],
    write_context: &'static str,
    close_context: &'static str,
) -> Result<bool, ExecutionError>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    match writer.write_all(input).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => return Ok(false),
        Err(error) => {
            return Err(ExecutionError::Infrastructure(
                anyhow::Error::new(error).context(write_context),
            ));
        }
    }

    match writer.shutdown().await {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(true),
        Err(error) => Err(ExecutionError::Infrastructure(
            anyhow::Error::new(error).context(close_context),
        )),
    }
}

#[derive(Clone, Debug)]
pub enum JobFileContent {
    Bytes(Bytes),
    LocalPath(PathBuf),
}

#[derive(Clone, Debug)]
pub struct JobFile {
    id: String,
    path: PathBuf,
    content: JobFileContent,
    executable: bool,
}

impl JobFile {
    pub fn executable(
        id: impl Into<String>,
        path: impl Into<PathBuf>,
        source: impl Into<PathBuf>,
    ) -> Result<Self, ExecutionError> {
        Self::new(id, path, JobFileContent::LocalPath(source.into()), true)
    }

    pub fn regular_file(
        id: impl Into<String>,
        path: impl Into<PathBuf>,
        source: impl Into<PathBuf>,
    ) -> Result<Self, ExecutionError> {
        Self::new(id, path, JobFileContent::LocalPath(source.into()), false)
    }

    pub fn bytes(
        id: impl Into<String>,
        path: impl Into<PathBuf>,
        content: impl Into<Bytes>,
    ) -> Result<Self, ExecutionError> {
        Self::new(id, path, JobFileContent::Bytes(content.into()), false)
    }

    fn new(
        id: impl Into<String>,
        path: impl Into<PathBuf>,
        content: JobFileContent,
        executable: bool,
    ) -> Result<Self, ExecutionError> {
        let id = id.into();
        if id.is_empty() {
            return Err(ExecutionError::InvalidInvocation(
                "job file id must not be empty".to_string(),
            ));
        }
        let path = path.into();
        validate_relative_path(&path)?;
        Ok(Self {
            id,
            path,
            content,
            executable,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn content(&self) -> &JobFileContent {
        &self.content
    }

    pub(crate) fn mode(&self) -> u32 {
        if self.executable {
            0o555
        } else {
            0o444
        }
    }

    pub(crate) fn is_executable(&self) -> bool {
        self.executable
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandArgument {
    Literal(String),
    FilePath(String),
    PrefixedFilePath { prefix: String, file_id: String },
}

impl CommandArgument {
    pub fn literal(value: impl Into<String>) -> Self {
        Self::Literal(value.into())
    }

    pub fn file(file_id: impl Into<String>) -> Self {
        Self::FilePath(file_id.into())
    }

    pub fn prefixed_file(prefix: impl Into<String>, file_id: impl Into<String>) -> Self {
        Self::PrefixedFilePath {
            prefix: prefix.into(),
            file_id: file_id.into(),
        }
    }

    fn file_id(&self) -> Option<&str> {
        match self {
            Self::Literal(_) => None,
            Self::FilePath(file_id) | Self::PrefixedFilePath { file_id, .. } => Some(file_id),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CompilerInvocation {
    program_id: String,
    args: Vec<CommandArgument>,
    stdin: Bytes,
    files: Vec<JobFile>,
    admission_permit: Option<Arc<OwnedSemaphorePermit>>,
}

impl CompilerInvocation {
    pub fn new(program: JobFile, args: Vec<CommandArgument>, stdin: impl Into<Bytes>) -> Self {
        let program_id = program.id.clone();
        Self {
            program_id,
            args,
            stdin: stdin.into(),
            files: vec![program],
            admission_permit: None,
        }
    }

    pub fn with_file(mut self, file: JobFile) -> Self {
        self.files.push(file);
        self
    }

    pub fn with_argument(mut self, argument: CommandArgument) -> Self {
        self.args.push(argument);
        self
    }

    pub fn validate(&self) -> Result<(), ExecutionError> {
        let mut ids = HashSet::new();
        let mut paths = HashSet::new();
        for file in &self.files {
            if !ids.insert(file.id.as_str()) {
                return Err(ExecutionError::InvalidInvocation(format!(
                    "duplicate job file id: {}",
                    file.id
                )));
            }
            if !paths.insert(file.path.as_path()) {
                return Err(ExecutionError::InvalidInvocation(format!(
                    "duplicate job file path: {}",
                    file.path.display()
                )));
            }
        }
        if !ids.contains(self.program_id.as_str()) {
            return Err(ExecutionError::InvalidInvocation(format!(
                "program file is missing: {}",
                self.program_id
            )));
        }
        for argument in &self.args {
            if let Some(file_id) = argument.file_id() {
                if !ids.contains(file_id) {
                    return Err(ExecutionError::InvalidInvocation(format!(
                        "argument references missing job file: {file_id}"
                    )));
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn program_path(&self, root: &Path) -> Result<PathBuf, ExecutionError> {
        self.file_path(root, &self.program_id, &HashMap::new())
    }

    #[cfg(test)]
    pub(crate) fn resolved_args(&self, root: &Path) -> Result<Vec<String>, ExecutionError> {
        self.args
            .iter()
            .map(|argument| match argument {
                CommandArgument::Literal(value) => Ok(value.clone()),
                CommandArgument::FilePath(file_id) => Ok(self
                    .file_path(root, file_id, &HashMap::new())?
                    .to_string_lossy()
                    .into_owned()),
                CommandArgument::PrefixedFilePath { prefix, file_id } => Ok(format!(
                    "{prefix}{}",
                    self.file_path(root, file_id, &HashMap::new())?
                        .to_string_lossy()
                )),
            })
            .collect()
    }

    pub(crate) fn program_path_with_overrides(
        &self,
        root: &Path,
        overrides: &HashMap<String, PathBuf>,
    ) -> Result<PathBuf, ExecutionError> {
        self.file_path(root, &self.program_id, overrides)
    }

    pub(crate) fn resolved_args_with_overrides(
        &self,
        root: &Path,
        overrides: &HashMap<String, PathBuf>,
    ) -> Result<Vec<String>, ExecutionError> {
        self.args
            .iter()
            .map(|argument| match argument {
                CommandArgument::Literal(value) => Ok(value.clone()),
                CommandArgument::FilePath(file_id) => Ok(self
                    .file_path(root, file_id, overrides)?
                    .to_string_lossy()
                    .into_owned()),
                CommandArgument::PrefixedFilePath { prefix, file_id } => Ok(format!(
                    "{prefix}{}",
                    self.file_path(root, file_id, overrides)?.to_string_lossy()
                )),
            })
            .collect()
    }

    fn file_path(
        &self,
        root: &Path,
        file_id: &str,
        overrides: &HashMap<String, PathBuf>,
    ) -> Result<PathBuf, ExecutionError> {
        let file = self
            .files
            .iter()
            .find(|file| file.id == file_id)
            .ok_or_else(|| {
                ExecutionError::InvalidInvocation(format!("job file does not exist: {file_id}"))
            })?;
        Ok(overrides
            .get(file_id)
            .cloned()
            .unwrap_or_else(|| root.join(&file.path)))
    }

    pub(crate) fn files(&self) -> &[JobFile] {
        &self.files
    }

    pub(crate) fn stdin(&self) -> &Bytes {
        &self.stdin
    }

    pub(crate) fn set_admission_permit(&mut self, permit: OwnedSemaphorePermit) {
        self.admission_permit = Some(Arc::new(permit));
    }

    pub(crate) fn admission_permit(&self) -> Option<Arc<OwnedSemaphorePermit>> {
        self.admission_permit.clone()
    }
}

#[derive(Clone, Debug)]
pub struct ExecutionOutput {
    pub stdout: Bytes,
    pub stderr: Bytes,
    pub exit_code: i64,
    pub oom_killed: bool,
}

impl ExecutionOutput {
    pub fn ensure_success(&self, compiler: &str) -> Result<(), ExecutionError> {
        if self.exit_code == 0 {
            Ok(())
        } else {
            Err(ExecutionError::CompilerFailed {
                compiler: compiler.to_string(),
                exit_code: self.exit_code,
                stderr: String::from_utf8_lossy(&self.stderr).into_owned(),
                oom_killed: self.oom_killed,
            })
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("invalid compiler invocation: {0}")]
    InvalidInvocation(String),
    #[error("compiler job input exceeds the configured limit of {limit} bytes")]
    UploadLimitExceeded { limit: u64 },
    #[error("compiler job output exceeds the configured limit of {limit} bytes")]
    OutputLimitExceeded { limit: usize },
    #[error("compiler job exceeded its {seconds}-second timeout")]
    Timeout { seconds: u64 },
    #[error("{compiler} exited with code {exit_code}; oom_killed={oom_killed}; stderr={stderr}")]
    CompilerFailed {
        compiler: String,
        exit_code: i64,
        stderr: String,
        oom_killed: bool,
    },
    #[error("compiler executor failed: {0:#}")]
    Infrastructure(#[from] anyhow::Error),
}

#[async_trait]
pub trait CompilerExecutor: Send + Sync {
    async fn execute(
        &self,
        invocation: CompilerInvocation,
    ) -> Result<ExecutionOutput, ExecutionError>;

    /// Work that must not hold an admission slot, such as filling a shared compiler cache.
    /// `ConcurrencyLimitedCompilerExecutor` runs it before admitting the invocation.
    async fn prepare(&self, _invocation: &CompilerInvocation) -> Result<(), ExecutionError> {
        Ok(())
    }

    async fn health_check(&self) -> Result<(), ExecutionError> {
        Ok(())
    }
}

fn validate_relative_path(path: &Path) -> Result<(), ExecutionError> {
    if path.as_os_str().is_empty() {
        return Err(ExecutionError::InvalidInvocation(
            "job file path must not be empty".to_string(),
        ));
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(ExecutionError::InvalidInvocation(format!(
                "job file path must be normalized and relative: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io,
        pin::Pin,
        task::{Context, Poll},
    };

    struct ScriptedWriter {
        write_error: Option<io::ErrorKind>,
        shutdown_error: Option<io::ErrorKind>,
        shutdown_polled: bool,
    }

    impl AsyncWrite for ScriptedWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            buffer: &[u8],
        ) -> Poll<io::Result<usize>> {
            match self.write_error.take() {
                Some(kind) => Poll::Ready(Err(io::Error::new(kind, "scripted write error"))),
                None => Poll::Ready(Ok(buffer.len())),
            }
        }

        fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            _context: &mut Context<'_>,
        ) -> Poll<io::Result<()>> {
            self.shutdown_polled = true;
            match self.shutdown_error.take() {
                Some(kind) => Poll::Ready(Err(io::Error::new(kind, "scripted close error"))),
                None => Poll::Ready(Ok(())),
            }
        }
    }

    fn writer(
        write_error: Option<io::ErrorKind>,
        shutdown_error: Option<io::ErrorKind>,
    ) -> ScriptedWriter {
        ScriptedWriter {
            write_error,
            shutdown_error,
            shutdown_polled: false,
        }
    }

    #[test]
    fn rejects_escaping_job_paths() {
        for path in ["", ".", "../compiler", "/compiler", "dir/../compiler"] {
            assert!(JobFile::bytes("compiler", path, Bytes::new()).is_err());
        }
    }

    #[test]
    fn validates_file_references() {
        let invocation = CompilerInvocation::new(
            JobFile::bytes("compiler", "bin/compiler", Bytes::new()).unwrap(),
            vec![CommandArgument::file("missing")],
            Bytes::new(),
        );
        assert!(invocation.validate().is_err());
    }

    #[tokio::test]
    async fn broken_pipe_is_only_ignored_for_peer_closed_stdin() {
        let mut write_broken = writer(Some(io::ErrorKind::BrokenPipe), None);
        assert!(
            !deliver_stdin(&mut write_broken, b"input", "write", "close")
                .await
                .unwrap()
        );
        assert!(!write_broken.shutdown_polled);

        let mut close_broken = writer(None, Some(io::ErrorKind::BrokenPipe));
        assert!(deliver_stdin(&mut close_broken, b"input", "write", "close")
            .await
            .unwrap());
        assert!(close_broken.shutdown_polled);

        let mut write_failed = writer(Some(io::ErrorKind::Other), None);
        let error = deliver_stdin(&mut write_failed, b"input", "write", "close")
            .await
            .unwrap_err();
        assert!(matches!(error, ExecutionError::Infrastructure(_)));
        assert!(error.to_string().contains("write"));

        let mut close_failed = writer(None, Some(io::ErrorKind::Other));
        let error = deliver_stdin(&mut close_failed, b"input", "write", "close")
            .await
            .unwrap_err();
        assert!(matches!(error, ExecutionError::Infrastructure(_)));
        assert!(error.to_string().contains("close"));
    }
}
