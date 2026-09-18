// SPDX-License-Identifier: LicenseRef-Blockscout

use super::{
    deliver_stdin, metrics, CompilerExecutor, CompilerInvocation, ExecutionError, ExecutionOutput,
    JobFile, JobFileContent, DEFAULT_EXECUTION_TIMEOUT_SECS, DEFAULT_MAX_OUTPUT_BYTES,
    DEFAULT_MAX_UPLOAD_BYTES,
};
use anyhow::Context;
use async_trait::async_trait;
use bytes::Bytes;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    time::timeout,
};

#[derive(Clone, Debug)]
pub struct NativeCompilerExecutor {
    execution_timeout: Duration,
    max_upload_bytes: u64,
    max_output_bytes: usize,
}

impl Default for NativeCompilerExecutor {
    fn default() -> Self {
        Self::new(
            Duration::from_secs(DEFAULT_EXECUTION_TIMEOUT_SECS),
            DEFAULT_MAX_OUTPUT_BYTES,
        )
        .expect("default native compiler limits are positive")
    }
}

impl NativeCompilerExecutor {
    pub fn new(
        execution_timeout: Duration,
        max_output_bytes: usize,
    ) -> Result<Self, ExecutionError> {
        if execution_timeout.is_zero() || max_output_bytes == 0 {
            return Err(ExecutionError::InvalidInvocation(
                "native compiler timeout and output limit must be positive".to_string(),
            ));
        }
        Ok(Self {
            execution_timeout,
            max_upload_bytes: DEFAULT_MAX_UPLOAD_BYTES,
            max_output_bytes,
        })
    }

    /// Stages job files under `root` and returns the executables that run in place instead.
    async fn stage(
        &self,
        invocation: &CompilerInvocation,
        root: &Path,
    ) -> Result<HashMap<String, PathBuf>, ExecutionError> {
        let mut total_size = invocation.stdin().len() as u64;
        let mut in_place = HashMap::new();
        for file in invocation.files() {
            if let Some(source) = in_place_executable(file).await? {
                in_place.insert(file.id().to_string(), source);
                continue;
            }
            let destination = root.join(file.path());
            if let Some(parent) = destination.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .with_context(|| format!("creating staging directory {}", parent.display()))?;
            }
            match file.content() {
                JobFileContent::Bytes(content) => {
                    total_size = total_size.saturating_add(content.len() as u64);
                    tokio::fs::write(&destination, content)
                        .await
                        .with_context(|| {
                            format!("writing staged file {}", destination.display())
                        })?;
                    metrics::add_transfer_bytes(
                        metrics::NATIVE,
                        metrics::INPUT,
                        "process",
                        content.len(),
                    );
                }
                JobFileContent::LocalPath(source) => {
                    let metadata = tokio::fs::metadata(source).await.with_context(|| {
                        format!("reading compiler artifact {}", source.display())
                    })?;
                    if !metadata.is_file() {
                        return Err(ExecutionError::InvalidInvocation(format!(
                            "job file source is not a regular file: {}",
                            source.display()
                        )));
                    }
                    total_size = total_size.saturating_add(metadata.len());
                    let copied =
                        tokio::fs::copy(source, &destination)
                            .await
                            .with_context(|| {
                                format!(
                                    "copying compiler artifact {} to {}",
                                    source.display(),
                                    destination.display()
                                )
                            })?;
                    metrics::add_transfer_bytes(
                        metrics::NATIVE,
                        metrics::INPUT,
                        "process",
                        copied as usize,
                    );
                }
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                tokio::fs::set_permissions(
                    &destination,
                    std::fs::Permissions::from_mode(file.mode()),
                )
                .await
                .with_context(|| format!("setting permissions on {}", destination.display()))?;
            }
            if total_size > self.max_upload_bytes {
                return Err(ExecutionError::UploadLimitExceeded {
                    limit: self.max_upload_bytes,
                });
            }
        }
        Ok(in_place)
    }

    async fn execute_inner(
        &self,
        invocation: CompilerInvocation,
    ) -> Result<ExecutionOutput, ExecutionError> {
        invocation.validate()?;
        let tempdir = {
            let _timer = metrics::start_operation(metrics::NATIVE, metrics::PREPARE, "process");
            ObservedTempDir::new(
                tempfile::tempdir().context("creating native compiler job directory")?,
            )
        };
        let in_place = {
            let _timer = metrics::start_operation(metrics::NATIVE, metrics::UPLOAD, "process");
            self.stage(&invocation, tempdir.path()).await?
        };

        let (mut child, process_group, mut child_stdin, child_stdout, child_stderr) = {
            let _timer = metrics::start_operation(metrics::NATIVE, metrics::CREATE, "process");
            let program = invocation.program_path_with_overrides(tempdir.path(), &in_place)?;
            let args = invocation.resolved_args_with_overrides(tempdir.path(), &in_place)?;
            let tmp = tempdir.path().join("tmp");
            tokio::fs::create_dir(&tmp)
                .await
                .context("creating compiler temporary directory")?;

            let mut command = Command::new(&program);
            command
                .args(args)
                .current_dir(tempdir.path())
                .env_clear()
                .env("HOME", &tmp)
                .env("TMPDIR", &tmp)
                .kill_on_drop(true)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            // The compiler leads its own process group, so the helpers it spawns can be killed
            // with it.
            #[cfg(unix)]
            command.process_group(0);

            let mut child = command
                .spawn()
                .with_context(|| format!("starting compiler {}", program.display()))?;
            let process_group = child.id();
            let child_stdin = child
                .stdin
                .take()
                .context("compiler stdin was not available")?;
            let child_stdout = child
                .stdout
                .take()
                .context("compiler stdout was not available")?;
            let child_stderr = child
                .stderr
                .take()
                .context("compiler stderr was not available")?;
            (
                child,
                process_group,
                child_stdin,
                child_stdout,
                child_stderr,
            )
        };

        let _timer = metrics::start_operation(metrics::NATIVE, metrics::EXECUTE, "process");
        let input = invocation.stdin().clone();
        let input_size = input.len();
        let output_size = Arc::new(AtomicUsize::new(0));
        let max_output_bytes = self.max_output_bytes;
        let execution = async move {
            let write_stdin = async move {
                let fully_written = deliver_stdin(
                    &mut child_stdin,
                    &input,
                    "writing compiler stdin",
                    "closing compiler stdin",
                )
                .await?;
                if fully_written {
                    metrics::add_transfer_bytes(
                        metrics::NATIVE,
                        metrics::INPUT,
                        "process",
                        input_size,
                    );
                }
                Ok::<(), ExecutionError>(())
            };
            let collect_stdout = collect_output(
                child_stdout,
                output_size.clone(),
                max_output_bytes,
                "stdout",
            );
            let collect_stderr =
                collect_output(child_stderr, output_size, max_output_bytes, "stderr");
            let wait = async move {
                child
                    .wait()
                    .await
                    .context("waiting for compiler")
                    .map_err(ExecutionError::Infrastructure)
            };
            let (_, stdout, stderr, status) =
                tokio::try_join!(write_stdin, collect_stdout, collect_stderr, wait)?;
            Ok::<_, ExecutionError>((stdout, stderr, status))
        };
        tokio::pin!(execution);
        // Declared after `execution`, so on timeout, error, or cancellation it drops first and
        // signals the group while the leader still holds it.
        let process_group = ProcessGroupKiller(process_group);

        let (stdout, stderr, status) = timeout(self.execution_timeout, &mut execution)
            .await
            .map_err(|_| ExecutionError::Timeout {
                seconds: self.execution_timeout.as_secs(),
            })??;
        process_group.disarm();

        Ok(ExecutionOutput {
            stdout,
            stderr,
            exit_code: status.code().map(i64::from).unwrap_or(-1),
            oom_killed: false,
        })
    }
}

/// Downloaded and preloaded compilers are already executable regular files, so they run in place
/// instead of being copied into every job directory. Other sources are still staged.
async fn in_place_executable(file: &JobFile) -> Result<Option<PathBuf>, ExecutionError> {
    let JobFileContent::LocalPath(source) = file.content() else {
        return Ok(None);
    };
    if !file.is_executable() {
        return Ok(None);
    }
    let metadata = tokio::fs::metadata(source)
        .await
        .with_context(|| format!("reading compiler artifact {}", source.display()))?;
    #[cfg(unix)]
    let executable = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    };
    #[cfg(not(unix))]
    let executable = false;
    if !metadata.is_file() || !executable {
        return Ok(None);
    }
    // The child runs in the job directory, so a relative program path would be ambiguous.
    let source = std::path::absolute(source)
        .with_context(|| format!("resolving compiler artifact {}", source.display()))?;
    Ok(Some(source))
}

/// Kills a compiler's whole process group unless disarmed. `kill_on_drop` only reaches the direct
/// child, and zksolc runs a separate process per contract.
struct ProcessGroupKiller(Option<u32>);

impl ProcessGroupKiller {
    fn disarm(mut self) {
        self.0 = None;
    }
}

impl Drop for ProcessGroupKiller {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(group) = self.0.and_then(|pid| libc::pid_t::try_from(pid).ok()) {
            // SAFETY: `kill` has no memory-safety preconditions. The group was created for this
            // job by spawning its leader with `process_group(0)`.
            unsafe {
                libc::kill(-group, libc::SIGKILL);
            }
        }
    }
}

struct ObservedTempDir(Option<tempfile::TempDir>);

impl ObservedTempDir {
    fn new(tempdir: tempfile::TempDir) -> Self {
        Self(Some(tempdir))
    }

    fn path(&self) -> &Path {
        self.0
            .as_ref()
            .expect("native compiler temporary directory is present")
            .path()
    }
}

impl Drop for ObservedTempDir {
    fn drop(&mut self) {
        let _timer = metrics::start_operation(metrics::NATIVE, metrics::CLEANUP, "process");
        drop(self.0.take());
    }
}

#[async_trait]
impl CompilerExecutor for NativeCompilerExecutor {
    async fn execute(
        &self,
        invocation: CompilerInvocation,
    ) -> Result<ExecutionOutput, ExecutionError> {
        let observation = metrics::JobObservation::new(metrics::NATIVE, "process");
        let result = self.execute_inner(invocation).await;
        observation.finish(&result);
        result
    }
}

async fn collect_output<R>(
    mut output: R,
    total_size: Arc<AtomicUsize>,
    limit: usize,
    stream_name: &'static str,
) -> Result<Bytes, ExecutionError>
where
    R: AsyncRead + Unpin,
{
    let mut collected = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let size = output
            .read(&mut buffer)
            .await
            .with_context(|| format!("reading compiler {stream_name}"))
            .map_err(ExecutionError::Infrastructure)?;
        if size == 0 {
            return Ok(Bytes::from(collected));
        }
        metrics::add_transfer_bytes(metrics::NATIVE, metrics::OUTPUT, "process", size);
        if total_size
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current
                    .checked_add(size)
                    .filter(|new_size| *new_size <= limit)
            })
            .is_err()
        {
            return Err(ExecutionError::OutputLimitExceeded { limit });
        }
        collected.extend_from_slice(&buffer[..size]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::runner::{CommandArgument, JobFile};

    #[cfg(unix)]
    #[tokio::test]
    async fn executes_direct_argv_with_staged_input() {
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        tokio::fs::write(&compiler, b"#!/bin/sh\n/bin/cat\n")
            .await
            .unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![CommandArgument::literal("-")],
            Bytes::from_static(b"standard-json"),
        );
        let output = NativeCompilerExecutor::default()
            .execute(invocation)
            .await
            .unwrap();
        output.ensure_success("test compiler").unwrap();
        assert_eq!(output.stdout, Bytes::from_static(b"standard-json"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn executable_sources_run_in_place_and_others_are_staged() {
        use std::os::unix::fs::PermissionsExt;

        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        tokio::fs::write(&compiler, b"#!/bin/sh\nprintf %s \"$0\"\n")
            .await
            .unwrap();
        let run = || async {
            let invocation = CompilerInvocation::new(
                JobFile::executable("compiler", "bin/compiler", &compiler).unwrap(),
                vec![],
                Bytes::new(),
            );
            let output = NativeCompilerExecutor::default()
                .execute(invocation)
                .await
                .unwrap();
            output.ensure_success("test compiler").unwrap();
            PathBuf::from(String::from_utf8(output.stdout.to_vec()).unwrap())
        };

        let staged = run().await;
        assert_ne!(staged, compiler);
        assert!(staged.ends_with("bin/compiler"));

        tokio::fs::set_permissions(&compiler, std::fs::Permissions::from_mode(0o555))
            .await
            .unwrap();
        assert_eq!(run().await, compiler);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn early_stdin_close_preserves_compiler_failure() {
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        tokio::fs::write(
            &compiler,
            b"#!/bin/sh\nexec 0<&-\nprintf 'fatal from compiler\\n' >&2\nexit 42\n",
        )
        .await
        .unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![],
            Bytes::from(vec![b'x'; 8 * 1024 * 1024]),
        );

        let output = NativeCompilerExecutor::default()
            .execute(invocation)
            .await
            .unwrap();
        assert_eq!(output.exit_code, 42);
        assert_eq!(output.stderr, Bytes::from_static(b"fatal from compiler\n"));
        assert!(matches!(
            output.ensure_success("test compiler"),
            Err(ExecutionError::CompilerFailed {
                exit_code: 42,
                ref stderr,
                ..
            }) if stderr == "fatal from compiler\n"
        ));
    }

    #[test]
    fn rejects_zero_native_limits() {
        assert!(NativeCompilerExecutor::new(Duration::ZERO, 1).is_err());
        assert!(NativeCompilerExecutor::new(Duration::from_secs(1), 0).is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn enforces_configured_execution_timeout() {
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        tokio::fs::write(&compiler, b"#!/bin/sh\n/bin/sleep 2\n")
            .await
            .unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![],
            Bytes::new(),
        );

        let error = NativeCompilerExecutor::new(Duration::from_secs(1), 1024)
            .unwrap()
            .execute(invocation)
            .await
            .unwrap_err();
        assert!(matches!(error, ExecutionError::Timeout { seconds: 1 }));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timeout_kills_processes_spawned_by_the_compiler() {
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        let pid_file = source_dir.path().join("helper.pid");
        tokio::fs::write(
            &compiler,
            b"#!/bin/sh\n/bin/sleep 60 &\necho $! > \"$1\"\nwait\n",
        )
        .await
        .unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![CommandArgument::literal(pid_file.to_str().unwrap())],
            Bytes::new(),
        );

        let error = NativeCompilerExecutor::new(Duration::from_secs(1), 1024)
            .unwrap()
            .execute(invocation)
            .await
            .unwrap_err();
        assert!(matches!(error, ExecutionError::Timeout { seconds: 1 }));

        let helper: libc::pid_t = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let helper_is_dead = || {
            // SAFETY: signal 0 only checks whether the process exists.
            if unsafe { libc::kill(helper, 0) } != 0 {
                return true;
            }
            // A killed helper may briefly remain a zombie until its new parent reaps it.
            std::process::Command::new("ps")
                .args(["-o", "stat=", "-p", &helper.to_string()])
                .output()
                .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).starts_with('Z'))
        };
        for _ in 0..50 {
            if helper_is_dead() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        // SAFETY: cleans up the helper so a failing test does not leak it.
        unsafe {
            libc::kill(helper, libc::SIGKILL);
        }
        panic!("a process spawned by the compiler outlived the timed-out job");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn enforces_and_can_raise_configured_output_limit() {
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        tokio::fs::write(&compiler, b"#!/bin/sh\nprintf 12345\n")
            .await
            .unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![],
            Bytes::new(),
        );

        let error = NativeCompilerExecutor::new(Duration::from_secs(5), 4)
            .unwrap()
            .execute(invocation.clone())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            ExecutionError::OutputLimitExceeded { limit: 4 }
        ));

        let output = NativeCompilerExecutor::new(Duration::from_secs(5), 5)
            .unwrap()
            .execute(invocation)
            .await
            .unwrap();
        assert_eq!(output.stdout, Bytes::from_static(b"12345"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stops_an_unbounded_output_stream_at_the_configured_limit() {
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        tokio::fs::write(
            &compiler,
            b"#!/bin/sh\nwhile :; do printf 12345678901234567890; done\n",
        )
        .await
        .unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![],
            Bytes::new(),
        );

        let error = timeout(
            Duration::from_secs(2),
            NativeCompilerExecutor::new(Duration::from_secs(30), 1024)
                .unwrap()
                .execute(invocation),
        )
        .await
        .expect("output limit must stop collection before the execution timeout")
        .unwrap_err();
        assert!(matches!(
            error,
            ExecutionError::OutputLimitExceeded { limit: 1024 }
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn output_limit_is_combined_across_stdout_and_stderr() {
        let source_dir = tempfile::tempdir().unwrap();
        let compiler = source_dir.path().join("compiler");
        tokio::fs::write(&compiler, b"#!/bin/sh\nprintf 123; printf 456 >&2\n")
            .await
            .unwrap();
        let invocation = CompilerInvocation::new(
            JobFile::executable("compiler", "bin/compiler", compiler).unwrap(),
            vec![],
            Bytes::new(),
        );

        let error = NativeCompilerExecutor::new(Duration::from_secs(5), 5)
            .unwrap()
            .execute(invocation.clone())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            ExecutionError::OutputLimitExceeded { limit: 5 }
        ));

        let output = NativeCompilerExecutor::new(Duration::from_secs(5), 6)
            .unwrap()
            .execute(invocation)
            .await
            .unwrap();
        assert_eq!(output.stdout, Bytes::from_static(b"123"));
        assert_eq!(output.stderr, Bytes::from_static(b"456"));
    }
}
