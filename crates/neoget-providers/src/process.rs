//! Process runner adapted from Neoenv's bounded concurrent-pipe/child-cleanup pattern.
//! Original project: Neoenv, MIT License, `/home/dev/work/plab/rs/apps/neoenv/lib/neoenv-runtime/src/remote/client.rs`.
//! This implementation independently adapts concurrent pipe draining, byte budgets, timeout polling,
//! and kill/reap cleanup; no Neoenv code or dependency is linked. See `THIRD_PARTY_NOTICES.md`.
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
const OUTPUT_LIMIT: usize = 8 * 1024 * 1024;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(300);
/// Safe executable plus argv; arguments are never interpreted by a shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Invocation {
    pub program: String,
    pub args: Vec<String>,
}
/// Output mode extension point; callers use `Stream` by default and no user setting is exposed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputPolicy {
    Stream,
    Capture,
}
/// A line delivered from one child output pipe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessEvent {
    Stdout(String),
    Stderr(String),
}
/// Injectable process boundary for adapters and deterministic tests.
pub trait ProcessRunner {
    fn run(
        &self,
        invocation: &Invocation,
        policy: OutputPolicy,
        sink: &mut dyn FnMut(ProcessEvent),
    ) -> Result<i32, String>;
}
/// Default production runner; concurrently drains both pipes, streams lines, caps each stream at 8 MiB,
/// and kills/reaps a child that times out or exceeds the output budget.
pub struct SystemRunner;
impl ProcessRunner for SystemRunner {
    fn run(
        &self,
        invocation: &Invocation,
        policy: OutputPolicy,
        sink: &mut dyn FnMut(ProcessEvent),
    ) -> Result<i32, String> {
        let mut child = Command::new(&invocation.program)
            .args(&invocation.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| format!("could not start {}", invocation.program))?;
        let stdout = child.stdout.take().ok_or("stdout pipe unavailable")?;
        let stderr = child.stderr.take().ok_or("stderr pipe unavailable")?;
        let (tx, rx) = mpsc::sync_channel::<PipeMessage>(128);
        let out_tx = tx.clone();
        let out = std::thread::spawn(move || drain(stdout, out_tx, true));
        let err = std::thread::spawn(move || drain(stderr, tx, false));
        let start = Instant::now();
        let mut exceeded = false;
        let status = loop {
            match rx.recv_timeout(Duration::from_millis(10)) {
                Ok(PipeMessage::Event(event)) => {
                    if policy == OutputPolicy::Stream {
                        sink(event)
                    }
                }
                Ok(PipeMessage::Limit) => {
                    exceeded = true;
                    terminate(&mut child);
                    break child
                        .wait()
                        .map_err(|_| "could not reap output-limited child")?;
                }
                Err(mpsc::RecvTimeoutError::Disconnected)
                | Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if start.elapsed() >= PROCESS_TIMEOUT {
                terminate(&mut child);
                let _ = child.wait();
                let _ = out.join();
                let _ = err.join();
                return Err("child process exceeded five-minute timeout".into());
            }
            if let Some(status) = child.try_wait().map_err(|_| {
                terminate(&mut child);
                "could not wait for child"
            })? {
                break status;
            }
        };
        let out_result = out.join().map_err(|_| "stdout reader failed")?;
        let err_result = err.join().map_err(|_| "stderr reader failed")?;
        // The main loop can observe process exit just before a final pipe message.
        for message in rx.try_iter() {
            match message {
                PipeMessage::Event(event) => {
                    if policy == OutputPolicy::Stream {
                        sink(event)
                    }
                }
                PipeMessage::Limit => exceeded = true,
            }
        }
        if exceeded || out_result.is_err() || err_result.is_err() {
            return Err("child output exceeded the 8 MiB per-stream limit".into());
        }
        Ok(status.code().unwrap_or(1))
    }
}
enum PipeMessage {
    Event(ProcessEvent),
    Limit,
}
fn drain(
    pipe: impl std::io::Read,
    tx: mpsc::SyncSender<PipeMessage>,
    stdout: bool,
) -> Result<(), ()> {
    let mut reader = BufReader::new(pipe);
    let mut line = String::new();
    let mut total = 0usize;
    loop {
        line.clear();
        let read = reader.read_line(&mut line).map_err(|_| ())?;
        if read == 0 {
            return Ok(());
        }
        total = total.saturating_add(read);
        if total > OUTPUT_LIMIT {
            let _ = tx.send(PipeMessage::Limit);
            return Err(());
        }
        let value = line.trim_end_matches(['\r', '\n']).to_owned();
        let event = if stdout {
            ProcessEvent::Stdout(value)
        } else {
            ProcessEvent::Stderr(value)
        };
        tx.send(PipeMessage::Event(event)).map_err(|_| ())?;
    }
}
fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
/// Explicit process context; default policy is streaming and the event sink is caller supplied.
pub struct ExecutionContext<'a> {
    runner: &'a dyn ProcessRunner,
    sink: &'a mut dyn FnMut(ProcessEvent),
}
static SYSTEM_RUNNER: SystemRunner = SystemRunner;
impl<'a> ExecutionContext<'a> {
    /// Uses the production streamed runner with the supplied event sink.
    pub fn system(sink: &'a mut dyn FnMut(ProcessEvent)) -> Self {
        Self::streaming(&SYSTEM_RUNNER, sink)
    }
    /// Creates a streamed execution context; callers retain ownership of runner and sink.
    pub fn streaming(runner: &'a dyn ProcessRunner, sink: &'a mut dyn FnMut(ProcessEvent)) -> Self {
        Self { runner, sink }
    }
    /// Runs one argv invocation and returns its exit code; nonzero codes are not converted to success.
    pub fn run(&mut self, invocation: &Invocation) -> Result<i32, String> {
        self.runner.run(invocation, OutputPolicy::Stream, self.sink)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_child_streams_both_pipes_before_exiting() {
        let started = Instant::now();
        let mut first_event_after = None;
        let mut events = vec![];
        #[cfg(unix)] let invocation=Invocation{program:"sh".into(),args:vec!["-c".into(),"printf 'out-first\\n'; printf 'err-first\\n' >&2; sleep 0.15; printf 'out-last\\n'; printf 'err-last\\n' >&2".into()]};
        #[cfg(windows)] let invocation=Invocation{program:"cmd".into(),args:vec!["/C".into(),"echo out-first & echo err-first 1>&2 & ping -n 2 127.0.0.1 >nul & echo out-last & echo err-last 1>&2".into()]};
        let code = SystemRunner
            .run(&invocation, OutputPolicy::Stream, &mut |event| {
                first_event_after.get_or_insert_with(|| started.elapsed());
                events.push(event)
            })
            .unwrap();
        assert_eq!(code, 0);
        assert!(events.contains(&ProcessEvent::Stdout("out-first".into())));
        assert!(events.contains(&ProcessEvent::Stderr("err-first".into())));
        assert!(events.contains(&ProcessEvent::Stdout("out-last".into())));
        assert!(events.contains(&ProcessEvent::Stderr("err-last".into())));
        assert!(first_event_after.unwrap() < Duration::from_millis(500));
    }
}
