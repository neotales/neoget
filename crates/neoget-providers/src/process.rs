//! Process runner adapted from Neoenv's bounded concurrent-pipe/child-cleanup pattern.
//! Original project: Neoenv, MIT License, `/home/dev/work/plab/rs/apps/neoenv/lib/neoenv-runtime/src/remote/client.rs`.
//! This implementation independently adapts concurrent pipe draining, byte budgets, timeout polling,
//! and kill/reap cleanup; no Neoenv code or dependency is linked. See `THIRD_PARTY_NOTICES.md`.
use std::{
    error::Error,
    fmt,
    io::Read,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
const OUTPUT_LIMIT: usize = 8 * 1024 * 1024;
const CHUNK_SIZE: usize = 4096;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(300);
const CLEANUP_BUDGET: Duration = Duration::from_secs(2);
/// Safe executable plus argv; arguments are never interpreted by a shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Invocation {
    pub program: String,
    pub args: Vec<String>,
}
/// A bounded, valid UTF-8 chunk delivered from one child pipe; chunks need not align to lines.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessEvent {
    Stdout(String),
    Stderr(String),
}
/// Cloneable cooperative cancellation signal for an in-flight child operation.
#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    /// Requests cancellation; the runner reacts on its next process poll.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
/// Stable process failure category, retaining an underlying I/O error when one exists.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ProcessErrorKind {
    ExecutableNotFound,
    PermissionDenied,
    SpawnIo,
    ReadIo,
    InvalidUtf8,
    Timeout,
    Cancelled,
    OutputLimit,
    Wait,
    Adapter,
}
/// Process failure; messages never echo argv values, which may contain sensitive inputs.
#[derive(Debug)]
pub struct ProcessError {
    kind: ProcessErrorKind,
    source: Option<std::io::Error>,
}
impl ProcessError {
    fn new(kind: ProcessErrorKind) -> Self {
        Self { kind, source: None }
    }
    fn io(kind: ProcessErrorKind, source: std::io::Error) -> Self {
        Self {
            kind,
            source: Some(source),
        }
    }
    pub(crate) fn adapter() -> Self {
        Self::new(ProcessErrorKind::Adapter)
    }
    /// Returns the stable machine-readable failure category.
    pub fn kind(&self) -> ProcessErrorKind {
        self.kind
    }
}
impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            ProcessErrorKind::ExecutableNotFound => "provider executable was not found",
            ProcessErrorKind::PermissionDenied => "provider executable permission was denied",
            ProcessErrorKind::SpawnIo => "provider process could not be started",
            ProcessErrorKind::ReadIo => "provider output could not be read",
            ProcessErrorKind::InvalidUtf8 => "provider output was not valid UTF-8",
            ProcessErrorKind::Timeout => "provider process exceeded its time budget",
            ProcessErrorKind::Cancelled => "provider process was cancelled",
            ProcessErrorKind::OutputLimit => "provider output exceeded its byte budget",
            ProcessErrorKind::Wait => "provider process could not be reaped",
            ProcessErrorKind::Adapter => "provider does not support the requested operation",
        })
    }
}
impl Error for ProcessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_ref().map(|e| e as _)
    }
}
/// Injectable process boundary for adapters and deterministic tests.
/// The callback is synchronous and may apply backpressure. Its panic propagates after direct-child cleanup.
/// Output is decoded as UTF-8; invalid data returns `InvalidUtf8`. A timeout/output failure can occur
/// after earlier events were delivered; the runner does not roll back external effects.
/// Killing/reaping the direct child is attempted; portable std
/// does not guarantee descendant process-tree termination or interruption of pipes inherited by descendants.
pub trait ProcessRunner {
    fn run(
        &self,
        invocation: &Invocation,
        cancellation: &CancellationToken,
        sink: &mut dyn FnMut(ProcessEvent),
    ) -> Result<i32, ProcessError>;
}
/// Default production runner: concurrent bounded byte-chunk readers, incremental stream events,
/// per-stream 8 MiB budgets, and a five-minute overall process deadline.
pub struct SystemRunner;
impl ProcessRunner for SystemRunner {
    fn run(
        &self,
        invocation: &Invocation,
        cancellation: &CancellationToken,
        sink: &mut dyn FnMut(ProcessEvent),
    ) -> Result<i32, ProcessError> {
        let process_started = Instant::now();
        let child = Command::new(&invocation.program)
            .args(&invocation.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {
                    ProcessError::io(ProcessErrorKind::ExecutableNotFound, e)
                }
                std::io::ErrorKind::PermissionDenied => {
                    ProcessError::io(ProcessErrorKind::PermissionDenied, e)
                }
                _ => ProcessError::io(ProcessErrorKind::SpawnIo, e),
            })?;
        let mut child_guard = ChildGuard::new(child);
        let stdout = child_guard
            .child_mut()
            .stdout
            .take()
            .ok_or_else(|| ProcessError::new(ProcessErrorKind::SpawnIo))?;
        let stderr = child_guard
            .child_mut()
            .stderr
            .take()
            .ok_or_else(|| ProcessError::new(ProcessErrorKind::SpawnIo))?;
        let (tx, rx) = mpsc::sync_channel::<Message>(64);
        let out = spawn_reader(stdout, tx.clone(), true);
        let err = spawn_reader(stderr, tx, false);
        let deadline = process_started + PROCESS_TIMEOUT;
        let cleanup_deadline = deadline + CLEANUP_BUDGET;
        let mut ended = 0;
        let mut result = None;
        let mut stdout_tail = Vec::new();
        let mut stderr_tail = Vec::new();
        while ended < 2 {
            let now = Instant::now();
            if cancellation.is_cancelled() {
                result = Some(ProcessError::new(ProcessErrorKind::Cancelled));
                break;
            }
            if now >= deadline {
                result = Some(ProcessError::new(ProcessErrorKind::Timeout));
                break;
            }
            match rx.recv_timeout((deadline - now).min(Duration::from_millis(20))) {
                Ok(Message::Chunk { stdout, bytes }) => {
                    let tail = if stdout {
                        &mut stdout_tail
                    } else {
                        &mut stderr_tail
                    };
                    match decode(bytes, stdout, tail, false) {
                        Ok(Some(event)) => sink(event),
                        Ok(None) => {}
                        Err(error) => {
                            result = Some(error);
                            break;
                        }
                    }
                }
                Ok(Message::End { stdout }) => {
                    let tail = if stdout {
                        &mut stdout_tail
                    } else {
                        &mut stderr_tail
                    };
                    match decode(Vec::new(), stdout, tail, true) {
                        Ok(Some(event)) => sink(event),
                        Ok(None) => {}
                        Err(error) => {
                            result = Some(error);
                            break;
                        }
                    }
                    ended += 1
                }
                Ok(Message::Failure(error)) => {
                    result = Some(error);
                    break;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    result = Some(ProcessError::new(ProcessErrorKind::ReadIo));
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
        // Continue waiting for the process only within the same overall deadline. Reader completion proves
        // pipes closed, while child wait polling avoids blocking beyond that deadline.
        let status = if result.is_none() {
            loop {
                if cancellation.is_cancelled() {
                    result = Some(ProcessError::new(ProcessErrorKind::Cancelled));
                    break None;
                }
                if Instant::now() >= deadline {
                    result = Some(ProcessError::new(ProcessErrorKind::Timeout));
                    break None;
                }
                match child_guard.child_mut().try_wait() {
                    Ok(Some(s)) => break Some(s),
                    Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                    Err(e) => {
                        result = Some(ProcessError::io(ProcessErrorKind::Wait, e));
                        break None;
                    }
                }
            }
        } else {
            None
        };
        if let Some(error) = result {
            child_guard.terminate_until(cleanup_deadline);
            drop(out);
            drop(err);
            return Err(error);
        }
        join_finished(out)?;
        join_finished(err)?;
        child_guard.disarm();
        Ok(status.and_then(|s| s.code()).unwrap_or(1))
    }
}
enum Message {
    Chunk { stdout: bool, bytes: Vec<u8> },
    End { stdout: bool },
    Failure(ProcessError),
}
fn spawn_reader(
    mut pipe: impl Read + Send + 'static,
    tx: mpsc::SyncSender<Message>,
    stdout: bool,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buf = [0u8; CHUNK_SIZE];
        let mut total = 0usize;
        loop {
            match pipe.read(&mut buf) {
                Ok(0) => {
                    let _ = tx.send(Message::End { stdout });
                    return;
                }
                Ok(n) => {
                    total = total.saturating_add(n);
                    if total > OUTPUT_LIMIT {
                        let _ = tx.send(Message::Failure(ProcessError::new(
                            ProcessErrorKind::OutputLimit,
                        )));
                        return;
                    }
                    if tx
                        .send(Message::Chunk {
                            stdout,
                            bytes: buf[..n].to_vec(),
                        })
                        .is_err()
                    {
                        return;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Message::Failure(ProcessError::io(
                        ProcessErrorKind::ReadIo,
                        e,
                    )));
                    return;
                }
            }
        }
    })
}
fn decode(
    bytes: Vec<u8>,
    stdout: bool,
    tail: &mut Vec<u8>,
    eof: bool,
) -> Result<Option<ProcessEvent>, ProcessError> {
    tail.extend_from_slice(&bytes);
    match std::str::from_utf8(tail) {
        Ok(text) => {
            let value = text.to_owned();
            tail.clear();
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(if stdout {
                    ProcessEvent::Stdout(value)
                } else {
                    ProcessEvent::Stderr(value)
                }))
            }
        }
        Err(error) => {
            if error.error_len().is_some() {
                return Err(ProcessError::new(ProcessErrorKind::InvalidUtf8));
            }
            let valid = error.valid_up_to();
            if eof {
                return Err(ProcessError::new(ProcessErrorKind::InvalidUtf8));
            }
            if valid == 0 {
                return Ok(None);
            }
            let rest = tail.split_off(valid);
            let text = String::from_utf8(std::mem::replace(tail, rest))
                .map_err(|_| ProcessError::new(ProcessErrorKind::InvalidUtf8))?;
            Ok(Some(if stdout {
                ProcessEvent::Stdout(text)
            } else {
                ProcessEvent::Stderr(text)
            }))
        }
    }
}
fn join_finished(handle: JoinHandle<()>) -> Result<(), ProcessError> {
    handle
        .join()
        .map_err(|_| ProcessError::new(ProcessErrorKind::ReadIo))
}
struct ChildGuard {
    child: Option<Child>,
}
impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }
    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("child guard armed")
    }
    fn disarm(&mut self) {
        self.child.take();
    }
    fn terminate_until(&mut self, deadline: Instant) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        self.child.take();
                        return;
                    }
                    _ if Instant::now() >= deadline => return,
                    _ => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        }
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.terminate_until(Instant::now() + CLEANUP_BUDGET);
    }
}
/// Explicit process context. It borrows a runner and event sink; `system` selects streamed production behavior.
pub struct ExecutionContext<'a> {
    runner: &'a dyn ProcessRunner,
    sink: &'a mut dyn FnMut(ProcessEvent),
    cancellation: CancellationToken,
}
static SYSTEM_RUNNER: SystemRunner = SystemRunner;
impl<'a> ExecutionContext<'a> {
    /// Creates a production streamed context using the supplied event sink.
    pub fn system(sink: &'a mut dyn FnMut(ProcessEvent)) -> Self {
        Self {
            runner: &SYSTEM_RUNNER,
            sink,
            cancellation: CancellationToken::default(),
        }
    }
    /// Creates a streamed context with an explicit runner, useful for controlled integration testing.
    pub fn streaming(runner: &'a dyn ProcessRunner, sink: &'a mut dyn FnMut(ProcessEvent)) -> Self {
        Self {
            runner,
            sink,
            cancellation: CancellationToken::default(),
        }
    }
    /// Returns a cloneable handle that can cancel this context's current or next run.
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }
    /// Executes one argv invocation. Exit status is process completion, not verified package success.
    /// Cancellation and timeout are checked between sink callbacks; a blocked callback delays response.
    pub fn run(&mut self, invocation: &Invocation) -> Result<i32, ProcessError> {
        self.runner.run(invocation, &self.cancellation, self.sink)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn command(script: &str) -> Invocation {
        #[cfg(unix)]
        {
            Invocation {
                program: "sh".into(),
                args: vec!["-c".into(), script.into()],
            }
        }
        #[cfg(windows)]
        {
            Invocation {
                program: "cmd".into(),
                args: vec!["/C".into(), script.into()],
            }
        }
    }
    #[cfg(unix)]
    #[test]
    fn streams_stdout_stderr_incrementally_before_exit() {
        let start = Instant::now();
        let mut first = None;
        let mut events = vec![];
        let code = SystemRunner
            .run(
                &command("printf 'out\\n'; printf 'err\\n' >&2; sleep 0.15"),
                &CancellationToken::default(),
                &mut |e| {
                    first.get_or_insert(start.elapsed());
                    events.push(e)
                },
            )
            .unwrap();
        assert_eq!(code, 0);
        assert!(first.unwrap() < Duration::from_millis(500));
        assert!(
            events
                .iter()
                .any(|e| matches!(e,ProcessEvent::Stdout(s)if s=="out\n"))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e,ProcessEvent::Stderr(s)if s=="err\n"))
        );
    }
    #[cfg(unix)]
    #[test]
    fn utf8_codepoints_split_across_chunks_are_reassembled() {
        let mut output = String::new();
        SystemRunner
            .run(
                &command("yes 'é' | head -n 3000"),
                &CancellationToken::default(),
                &mut |event| {
                    if let ProcessEvent::Stdout(chunk) = event {
                        output.push_str(&chunk);
                    }
                },
            )
            .unwrap();
        assert_eq!(output.matches('é').count(), 3000);
    }
    #[cfg(unix)]
    #[test]
    fn invalid_utf8_has_a_distinct_error_category() {
        let error = SystemRunner
            .run(
                &command("printf '\\377'"),
                &CancellationToken::default(),
                &mut |_| {},
            )
            .unwrap_err();
        assert_eq!(error.kind(), ProcessErrorKind::InvalidUtf8);
    }
    #[cfg(unix)]
    #[test]
    fn slow_sink_does_not_deadlock_reader_joins_after_fast_child_output() {
        let mut events = 0;
        let result = SystemRunner.run(
            &command("yes x | head -c 300000"),
            &CancellationToken::default(),
            &mut |_| {
                events += 1;
                std::thread::sleep(Duration::from_millis(1))
            },
        );
        assert_eq!(result.unwrap(), 0);
        assert!(events > 64);
    }
    #[cfg(unix)]
    #[test]
    fn oversized_unterminated_output_is_bounded_and_fails() {
        let error = SystemRunner
            .run(
                &command("yes x | tr -d '\\n' | head -c 9000000"),
                &CancellationToken::default(),
                &mut |_| {},
            )
            .unwrap_err();
        assert_eq!(error.kind(), ProcessErrorKind::OutputLimit);
    }
    #[cfg(unix)]
    #[test]
    fn sink_panic_kills_and_reaps_direct_child() {
        let started = Instant::now();
        let mut child_pid = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = SystemRunner.run(
                &command("printf '%s\\n' $$; exec sleep 30"),
                &CancellationToken::default(),
                &mut |event| {
                    if let ProcessEvent::Stdout(pid) = event {
                        child_pid = pid.trim().parse::<u32>().ok();
                    }
                    panic!("sink failure");
                },
            );
        }));
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
        #[cfg(target_os = "linux")]
        assert!(!std::path::Path::new(&format!("/proc/{}", child_pid.unwrap())).exists());
    }
    #[cfg(unix)]
    #[test]
    fn cancellation_kills_the_direct_child() {
        let token = CancellationToken::default();
        let cancellation = token.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            cancellation.cancel();
        });
        let error = SystemRunner
            .run(&command("exec sleep 30"), &token, &mut |_| {})
            .unwrap_err();
        assert_eq!(error.kind(), ProcessErrorKind::Cancelled);
    }
    #[cfg(unix)]
    #[test]
    fn cancellation_is_checked_after_pipes_close_while_child_remains() {
        let token = CancellationToken::default();
        let cancellation = token.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            cancellation.cancel();
        });
        let error = SystemRunner
            .run(
                &command("exec sleep 30 >/dev/null 2>&1"),
                &token,
                &mut |_| {},
            )
            .unwrap_err();
        assert_eq!(error.kind(), ProcessErrorKind::Cancelled);
    }
    #[test]
    fn public_context_example_runs_a_safe_argv_command() {
        struct TestRunner;
        impl ProcessRunner for TestRunner {
            fn run(
                &self,
                _: &Invocation,
                _: &CancellationToken,
                sink: &mut dyn FnMut(ProcessEvent),
            ) -> Result<i32, ProcessError> {
                sink(ProcessEvent::Stdout("example\n".into()));
                Ok(0)
            }
        }
        let mut received = vec![];
        let mut sink = |e| received.push(e);
        let runner = TestRunner;
        let mut context = ExecutionContext::streaming(&runner, &mut sink);
        let status = context
            .run(&Invocation {
                program: "printf".into(),
                args: vec!["example".into()],
            })
            .unwrap();
        assert_eq!(status, 0);
        assert!(
            received
                .iter()
                .any(|e| matches!(e,ProcessEvent::Stdout(s)if s=="example\n"))
        );
    }
    #[test]
    fn missing_executable_has_a_stable_typed_error_and_io_source() {
        let error = SystemRunner
            .run(
                &Invocation {
                    program: "neoget-executable-that-does-not-exist".into(),
                    args: vec![],
                },
                &CancellationToken::default(),
                &mut |_| {},
            )
            .unwrap_err();
        assert_eq!(error.kind(), ProcessErrorKind::ExecutableNotFound);
        assert!(error.source().is_some());
    }
    #[cfg(unix)]
    #[test]
    fn non_executable_path_has_a_permission_error_and_io_source() {
        use std::os::unix::fs::PermissionsExt;
        let path =
            std::env::temp_dir().join(format!("neoget-non-executable-{}", std::process::id()));
        std::fs::write(&path, b"not executable").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let error = SystemRunner
            .run(
                &Invocation {
                    program: path.to_string_lossy().into_owned(),
                    args: vec![],
                },
                &CancellationToken::default(),
                &mut |_| {},
            )
            .unwrap_err();
        std::fs::remove_file(path).unwrap();
        assert_eq!(error.kind(), ProcessErrorKind::PermissionDenied);
        assert!(error.source().is_some());
    }
}
