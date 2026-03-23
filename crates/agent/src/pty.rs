use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, error};

use crate::traits::{AgentError, AgentOutput};

pub struct PtyProcess {
    output_tx: broadcast::Sender<AgentOutput>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    running: Arc<AtomicBool>,
    collected_output: Arc<Mutex<Vec<u8>>>,
    reader_handle: Option<tokio::task::JoinHandle<()>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
}

impl PtyProcess {
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

        let (output_tx, _) = broadcast::channel(1024);
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
            writer: Arc::new(Mutex::new(writer)),
            running,
            collected_output,
            reader_handle: Some(reader_handle),
            child: Arc::new(Mutex::new(child)),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentOutput> {
        self.output_tx.subscribe()
    }

    pub async fn write_stdin(&self, data: &[u8]) -> Result<(), AgentError> {
        let mut writer = self.writer.lock().await;
        writer.write_all(data).map_err(AgentError::Io)?;
        writer.flush().map_err(AgentError::Io)?;
        Ok(())
    }

    pub async fn wait(&mut self) -> Result<i32, AgentError> {
        let mut child = self.child.lock().await;
        let status = tokio::task::block_in_place(|| child.wait())
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

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub async fn collected_output(&self) -> String {
        let collected = self.collected_output.lock().await;
        String::from_utf8_lossy(&collected).to_string()
    }

    pub fn collected_output_sync(&self) -> String {
        match self.collected_output.try_lock() {
            Ok(collected) => String::from_utf8_lossy(&collected).to_string(),
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
        cmd.arg("hello conductor");
        let mut proc = PtyProcess::spawn(cmd).unwrap();
        let exit_code = proc.wait().await.unwrap();
        assert_eq!(exit_code, 0);
        let output = proc.collected_output().await;
        assert!(output.contains("hello conductor"), "output was: {output}");
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
