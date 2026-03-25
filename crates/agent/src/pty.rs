use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use strip_ansi_escapes::strip as strip_ansi;
use tokio::sync::{Mutex, broadcast};
use tracing::{debug, error};

use crate::traits::{AgentError, AgentOutput};

/// Handle that allows resizing a PTY from any thread.
#[derive(Clone)]
pub struct PtyResizer {
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
}

impl PtyResizer {
    pub fn resize(&self, cols: u16, rows: u16) {
        tracing::debug!(
            "PtyResizer::resize called with cols={}, rows={}",
            cols,
            rows
        );
        if let Ok(master) = self.master.try_lock() {
            let result = master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
            tracing::debug!("PtyResizer::resize result: {:?}", result);
        } else {
            tracing::warn!("PtyResizer::resize: failed to acquire master lock");
        }
    }
}

static RESIZERS: OnceLock<std::sync::Mutex<HashMap<String, PtyResizer>>> = OnceLock::new();

fn resizer_registry() -> &'static std::sync::Mutex<HashMap<String, PtyResizer>> {
    RESIZERS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Register a resizer handle for a step so the UI can look it up by name.
pub fn register_resizer(id: &str, resizer: PtyResizer) {
    if let Ok(mut map) = resizer_registry().lock() {
        tracing::debug!("register_resizer: registering key={:?}", id);
        map.insert(id.to_string(), resizer);
    }
}

/// Retrieve a previously registered resizer by step name.
pub fn get_resizer(id: &str) -> Option<PtyResizer> {
    let map = resizer_registry().lock().ok()?;
    let keys: Vec<&String> = map.keys().collect();
    let result = map.get(id).cloned();
    tracing::debug!(
        "get_resizer: lookup key={:?}, found={}, registry_keys={:?}",
        id,
        result.is_some(),
        keys
    );
    result
}

/// Remove a resizer when it is no longer needed.
pub fn remove_resizer(id: &str) {
    if let Ok(mut map) = resizer_registry().lock() {
        map.remove(id);
    }
}

pub struct PtyProcess {
    output_tx: broadcast::Sender<AgentOutput>,
    initial_rx: Option<broadcast::Receiver<AgentOutput>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    running: Arc<AtomicBool>,
    collected_output: Arc<Mutex<Vec<u8>>>,
    reader_handle: Option<tokio::task::JoinHandle<()>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
}

impl PtyProcess {
    /// Async-safe wrapper that runs the blocking PTY spawn on a dedicated thread.
    pub async fn spawn_async(cmd: CommandBuilder) -> Result<Self, AgentError> {
        tokio::task::spawn_blocking(move || Self::spawn(cmd))
            .await
            .map_err(|e| AgentError::SpawnError(format!("spawn task failed: {e}")))?
    }

    pub fn spawn(cmd: CommandBuilder) -> Result<Self, AgentError> {
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| AgentError::SpawnError(format!("failed to open PTY: {e}")))?;

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| AgentError::SpawnError(format!("failed to spawn command: {e}")))?;

        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| AgentError::SpawnError(format!("failed to clone PTY reader: {e}")))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| AgentError::SpawnError(format!("failed to take PTY writer: {e}")))?;

        let master = Arc::new(Mutex::new(pair.master));

        let (output_tx, initial_rx) = broadcast::channel(1024);
        let running = Arc::new(AtomicBool::new(true));
        let collected_output = Arc::new(Mutex::new(Vec::new()));

        let tx = output_tx.clone();
        let is_running = running.clone();
        let collected = collected_output.clone();
        let reader_handle = tokio::task::spawn_blocking(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        debug!("PTY reader got EOF");
                        break;
                    }
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        if let Ok(mut collected) = collected.try_lock() {
                            collected.extend_from_slice(&data);
                        }
                        let _ = tx.send(AgentOutput::Stdout(data));
                    }
                    Err(e) => {
                        if is_running.load(Ordering::Relaxed) {
                            error!("PTY read error: {e}");
                        }
                        break;
                    }
                }
            }
            is_running.store(false, Ordering::Relaxed);
        });

        Ok(Self {
            output_tx,
            initial_rx: Some(initial_rx),
            writer: Arc::new(Mutex::new(writer)),
            master,
            running,
            collected_output,
            reader_handle: Some(reader_handle),
            child: Arc::new(Mutex::new(child)),
        })
    }

    /// Returns a cloneable handle for resizing this PTY from any thread.
    pub fn resizer(&self) -> PtyResizer {
        PtyResizer {
            master: self.master.clone(),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentOutput> { self.output_tx.subscribe() }

    /// Returns the pre-subscribed receiver that was created before the reader
    /// thread started, guaranteeing no output is missed. Returns `None` if
    /// already taken.
    pub(crate) fn take_initial_receiver(&mut self) -> Option<broadcast::Receiver<AgentOutput>> {
        self.initial_rx.take()
    }

    pub async fn write_stdin(&self, data: &[u8]) -> Result<(), AgentError> {
        let mut writer = self.writer.lock().await;
        writer.write_all(data).map_err(AgentError::Io)?;
        writer.flush().map_err(AgentError::Io)?;
        Ok(())
    }

    pub async fn resize(&self, cols: u16, rows: u16) -> Result<(), AgentError> {
        let master = self.master.clone();
        tokio::task::spawn_blocking(move || {
            let master = master.blocking_lock();
            master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
        })
        .await
        .map_err(|e| AgentError::ExecutionError(format!("resize task failed: {e}")))?
        .map_err(|e| AgentError::ExecutionError(format!("failed to resize PTY: {e}")))
    }

    pub async fn wait(&mut self) -> Result<i32, AgentError> {
        let child = self.child.clone();
        let status = tokio::task::spawn_blocking(move || {
            let mut child = child.blocking_lock();
            child.wait()
        })
        .await
        .map_err(|e| AgentError::ExecutionError(format!("wait task failed: {e}")))?
        .map_err(|e| AgentError::ExecutionError(format!("failed to wait: {e}")))?;

        self.running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.await;
        }

        let code: i32 = status.exit_code().try_into().unwrap_or(-1);
        let _ = self.output_tx.send(AgentOutput::Exited(code));
        Ok(code)
    }

    pub async fn kill(&mut self) -> Result<(), AgentError> {
        let mut child = self.child.lock().await;
        child
            .kill()
            .map_err(|e| AgentError::ExecutionError(format!("failed to kill: {e}")))?;
        self.running.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub fn is_running(&self) -> bool { self.running.load(Ordering::Relaxed) }

    pub async fn collected_output(&self) -> String {
        let collected = self.collected_output.lock().await;
        let stripped = strip_ansi(&*collected);
        String::from_utf8_lossy(&stripped).to_string()
    }

    pub fn collected_output_sync(&self) -> String {
        match self.collected_output.try_lock() {
            Ok(collected) => {
                let stripped = strip_ansi(&*collected);
                String::from_utf8_lossy(&stripped).to_string()
            }
            Err(_) => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_spawn_echo() {
        let mut cmd = CommandBuilder::new("echo");
        cmd.arg("hello fridi");
        let mut proc = PtyProcess::spawn(cmd).unwrap();
        let exit_code = proc.wait().await.unwrap();
        assert_eq!(exit_code, 0);
        let output = proc.collected_output().await;
        assert!(output.contains("hello fridi"), "output was: {output}");
    }

    #[tokio::test]
    async fn test_spawn_cat_stdin() {
        let cmd = CommandBuilder::new("cat");
        let mut proc = PtyProcess::spawn(cmd).unwrap();
        proc.write_stdin(b"hello from stdin\n").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        proc.kill().await.unwrap();
        let output = proc.collected_output().await;
        assert!(output.contains("hello from stdin"), "output was: {output}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_broadcast_subscriber_receives_output() {
        let mut cmd = CommandBuilder::new("echo");
        cmd.arg("hello world");
        let mut proc = PtyProcess::spawn(cmd).unwrap();

        // Subscribe BEFORE the process produces output
        let mut rx = proc.subscribe();

        let exit_code = proc.wait().await.unwrap();
        assert_eq!(exit_code, 0);

        // Collect all Stdout events from the broadcast channel
        let mut received = Vec::new();
        while let Ok(output) = rx.try_recv() {
            if let AgentOutput::Stdout(data) = output {
                received.extend_from_slice(&data);
            }
        }
        let text = String::from_utf8_lossy(&received);
        assert!(
            text.contains("hello world"),
            "broadcast subscriber should receive 'hello world', got: {text}"
        );

        // Also verify collected_output matches
        let collected = proc.collected_output().await;
        assert!(
            collected.contains("hello world"),
            "collected_output should contain 'hello world', got: {collected}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_initial_receiver_captures_all_output() {
        let mut cmd = CommandBuilder::new("echo");
        cmd.arg("hello late");
        let mut proc = PtyProcess::spawn(cmd).unwrap();

        // Use the pre-subscribed receiver instead of a late subscribe
        let mut rx = proc
            .take_initial_receiver()
            .expect("initial receiver should be available");

        let exit_code = proc.wait().await.unwrap();
        assert_eq!(exit_code, 0);

        let mut received = Vec::new();
        while let Ok(output) = rx.try_recv() {
            if let AgentOutput::Stdout(data) = output {
                received.extend_from_slice(&data);
            }
        }
        let text = String::from_utf8_lossy(&received);

        let collected = proc.collected_output().await;
        assert!(
            collected.contains("hello late"),
            "collected_output should always contain data, got: {collected}"
        );

        assert!(
            text.contains("hello late"),
            "initial receiver should receive 'hello late', got: {text} \
             (collected_output has it: {collected})"
        );

        // Second call returns None since the receiver was already taken
        assert!(
            proc.take_initial_receiver().is_none(),
            "take_initial_receiver should return None after first call"
        );
    }

    #[tokio::test]
    async fn test_is_running() {
        let mut cmd = CommandBuilder::new("sleep");
        cmd.arg("10");
        let mut proc = PtyProcess::spawn(cmd).unwrap();
        assert!(proc.is_running());
        proc.kill().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(!proc.is_running());
    }
}
