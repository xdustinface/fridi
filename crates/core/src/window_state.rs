use std::collections::HashMap;
use std::path::Path;
use std::{fs, io};

use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowState {
    pub windows: HashMap<String, WindowInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub open_sessions: Vec<String>,
    pub active_tab: Option<String>,
}

impl WindowState {
    pub fn load(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        let tmp_name = format!(
            "{}.{:016x}.tmp",
            path.file_name().unwrap_or_default().to_string_lossy(),
            rand::thread_rng().gen::<u64>()
        );
        let tmp_path = path.with_file_name(tmp_name);
        fs::write(&tmp_path, &json)?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    }

    pub fn update_tab(&mut self, repo: &str, session_id: &str, is_open: bool) {
        let info = self
            .windows
            .entry(repo.to_string())
            .or_insert_with(|| WindowInfo {
                open_sessions: Vec::new(),
                active_tab: None,
            });

        if is_open {
            if !info.open_sessions.iter().any(|s| s == session_id) {
                info.open_sessions.push(session_id.to_string());
            }
        } else {
            info.open_sessions.retain(|s| s != session_id);
            if info.active_tab.as_deref() == Some(session_id) {
                info.active_tab = info.open_sessions.last().cloned();
            }
        }
    }

    pub fn set_active(&mut self, repo: &str, session_id: &str) {
        if let Some(info) = self.windows.get_mut(repo) {
            if info.open_sessions.iter().any(|s| s == session_id) {
                info.active_tab = Some(session_id.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_window_state_save_load() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("fridi-state.json");

        let mut state = WindowState::default();
        state.update_tab("owner/repo", "session-1", true);
        state.update_tab("owner/repo", "session-2", true);
        state.set_active("owner/repo", "session-2");

        state.save(&path).unwrap();
        let loaded = WindowState::load(&path);

        let info = loaded.windows.get("owner/repo").unwrap();
        assert_eq!(info.open_sessions, vec!["session-1", "session-2"]);
        assert_eq!(info.active_tab.as_deref(), Some("session-2"));
    }

    #[test]
    fn test_window_state_update_tab() {
        let mut state = WindowState::default();

        state.update_tab("owner/repo", "s1", true);
        state.update_tab("owner/repo", "s2", true);
        assert_eq!(state.windows["owner/repo"].open_sessions, vec!["s1", "s2"]);

        // Adding a duplicate does nothing
        state.update_tab("owner/repo", "s1", true);
        assert_eq!(state.windows["owner/repo"].open_sessions, vec!["s1", "s2"]);

        // Removing a tab
        state.update_tab("owner/repo", "s1", false);
        assert_eq!(state.windows["owner/repo"].open_sessions, vec!["s2"]);
    }

    #[test]
    fn test_window_state_set_active() {
        let mut state = WindowState::default();
        state.update_tab("owner/repo", "s1", true);
        state.update_tab("owner/repo", "s2", true);

        state.set_active("owner/repo", "s1");
        assert_eq!(
            state.windows["owner/repo"].active_tab.as_deref(),
            Some("s1")
        );

        state.set_active("owner/repo", "s2");
        assert_eq!(
            state.windows["owner/repo"].active_tab.as_deref(),
            Some("s2")
        );

        // Setting active on a session that is not open does nothing
        state.set_active("owner/repo", "s3");
        assert_eq!(
            state.windows["owner/repo"].active_tab.as_deref(),
            Some("s2")
        );
    }

    #[test]
    fn test_window_state_missing_file() {
        let state = WindowState::load(Path::new("/tmp/fridi-nonexistent-state-xyz.json"));
        assert!(state.windows.is_empty());
    }

    #[test]
    fn test_window_state_close_active_tab_selects_last() {
        let mut state = WindowState::default();
        state.update_tab("r", "s1", true);
        state.update_tab("r", "s2", true);
        state.update_tab("r", "s3", true);
        state.set_active("r", "s2");

        // Closing the active tab should select the last remaining session
        state.update_tab("r", "s2", false);
        assert_eq!(state.windows["r"].active_tab.as_deref(), Some("s3"));
    }

    #[test]
    fn test_window_state_atomic_write() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("fridi-state.json");

        let state = WindowState::default();
        state.save(&path).unwrap();

        // No .tmp files should remain
        let tmp_files: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(tmp_files.is_empty());
        assert!(path.exists());
    }
}
