pub(crate) const APP_CSS: &str = r#"
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    background-color: #161b22;
    color: #e6edf3;
    line-height: 1.5;
}

.app-layout {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
}

/* Tab bar */
.tab-bar {
    display: flex;
    align-items: center;
    background-color: #0d1117;
    border-bottom: 1px solid #30363d;
    min-height: 40px;
    padding: 0 4px;
    overflow-x: auto;
    flex-shrink: 0;
}

.tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    cursor: pointer;
    border: 1px solid transparent;
    border-bottom: none;
    border-radius: 6px 6px 0 0;
    margin-right: 2px;
    font-size: 13px;
    color: #8b949e;
    transition: background-color 0.15s, color 0.15s;
    white-space: nowrap;
    max-width: 200px;
}

.tab:hover {
    background-color: #1c2128;
    color: #e6edf3;
}

.tab.active {
    background-color: #161b22;
    border-color: #5f875f;
    color: #e6edf3;
}

.tab-name {
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
}

.tab-session-id {
    font-size: 10px;
    color: #8b949e;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 80px;
}

.tab-close {
    background: none;
    border: none;
    color: #8b949e;
    cursor: pointer;
    font-size: 12px;
    padding: 0 2px;
    line-height: 1;
    border-radius: 3px;
}

.tab-close:hover {
    background-color: #30363d;
    color: #e6edf3;
}

.tab-new {
    background: none;
    border: 1px solid #30363d;
    color: #8b949e;
    cursor: pointer;
    font-size: 16px;
    padding: 4px 10px;
    margin-left: 4px;
    border-radius: 6px;
    transition: background-color 0.15s, color 0.15s;
}

.tab-new:hover {
    background-color: #1c2128;
    color: #5f875f;
}

/* Status indicators */
.status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
}

.status-dot.idle {
    background-color: #808080;
}

.status-dot.pending {
    background-color: #808080;
}

.status-dot.running {
    background-color: #5f875f;
    animation: pulse 1.5s ease-in-out infinite;
}

.status-dot.completed {
    background-color: #87af87;
}

.status-dot.failed {
    background-color: #d78787;
}

@keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
}

/* Main content */
.main-content {
    flex: 1;
    overflow: hidden;
}

.empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #8b949e;
    font-size: 16px;
}

/* Workflow view */
.workflow-header {
    margin-bottom: 24px;
}

.workflow-header h2 {
    font-size: 24px;
    font-weight: 600;
    color: #e6edf3;
}

.workflow-header p {
    font-size: 14px;
    color: #8b949e;
    margin-top: 4px;
}

.workflow-meta {
    display: flex;
    gap: 12px;
    margin-top: 12px;
    flex-wrap: wrap;
}

.meta-tag {
    font-size: 12px;
    padding: 4px 10px;
    background-color: #1c2128;
    border: 1px solid #30363d;
    border-radius: 4px;
    color: #8b949e;
}

.steps-section h3 {
    font-size: 16px;
    font-weight: 500;
    color: #e6edf3;
    margin-bottom: 12px;
}

.steps-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
}

/* Step card */
.step-card {
    background-color: #1c2128;
    border: 1px solid #30363d;
    border-radius: 8px;
    padding: 16px;
    transition: border-color 0.15s;
    cursor: pointer;
}

.step-card:hover {
    border-color: #5f875f;
}

.step-card.running {
    border-color: #5f875f;
}

.step-card.completed {
    border-color: #87af87;
}

.step-card.failed {
    border-color: #d78787;
}

.step-card.skipped {
    opacity: 0.5;
}

.step-card-header {
    display: flex;
    align-items: center;
    gap: 10px;
}

.step-name {
    font-size: 14px;
    font-weight: 500;
    color: #e6edf3;
    flex: 1;
}

.step-status-text {
    font-size: 12px;
    font-weight: 500;
}

.step-status-text.pending { color: #808080; }
.step-status-text.running { color: #5f875f; }
.step-status-text.completed { color: #87af87; }
.step-status-text.failed { color: #d78787; }
.step-status-text.skipped { color: #808080; }

.step-details {
    margin-top: 8px;
    font-size: 12px;
    color: #8b949e;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
}

.step-detail-label {
    color: #808080;
}

.step-deps {
    margin-top: 6px;
    font-size: 11px;
    color: #808080;
}

/* Workflow picker modal */
.picker-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(13, 17, 23, 0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
}

.picker-modal {
    background-color: #1c2128;
    border: 1px solid #30363d;
    border-radius: 12px;
    padding: 24px;
    min-width: 360px;
    max-width: 480px;
    max-height: 60vh;
    display: flex;
    flex-direction: column;
}

.picker-modal h3 {
    font-size: 18px;
    font-weight: 600;
    color: #e6edf3;
    margin-bottom: 16px;
}

.picker-list {
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.picker-item {
    padding: 12px;
    border-radius: 8px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background-color 0.15s, border-color 0.15s;
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.picker-item:hover {
    background-color: #30363d;
    border-color: #5f875f;
}

.picker-item-name {
    font-size: 14px;
    font-weight: 500;
    color: #e6edf3;
}

.picker-item-desc {
    font-size: 12px;
    color: #8b949e;
}

.picker-empty {
    padding: 24px;
    text-align: center;
    color: #8b949e;
    font-size: 14px;
}

/* Session creator */
.session-creator {
    min-width: 420px;
    max-width: 520px;
}

.mode-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin-bottom: 12px;
}

.mode-btn {
    background-color: #0d1117;
    border: 1px solid #30363d;
    border-radius: 8px;
    padding: 16px 12px;
    cursor: pointer;
    text-align: left;
    color: #e6edf3;
    transition: background-color 0.15s, border-color 0.15s;
}

.mode-btn:hover:not(:disabled) {
    background-color: #30363d;
    border-color: #5f875f;
}

.mode-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
}

.mode-btn-title {
    font-size: 14px;
    font-weight: 600;
    margin-bottom: 4px;
}

.mode-btn-desc {
    font-size: 12px;
    color: #8b949e;
}

.creator-header {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 16px;
}

.creator-header h3 {
    margin-bottom: 0;
}

.creator-back {
    background: none;
    border: 1px solid #30363d;
    color: #8b949e;
    cursor: pointer;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 6px;
    transition: background-color 0.15s, color 0.15s;
}

.creator-back:hover {
    background-color: #30363d;
    color: #e6edf3;
}

.creator-search {
    width: 100%;
    padding: 10px 12px;
    margin-bottom: 12px;
    background-color: #0d1117;
    border: 1px solid #30363d;
    border-radius: 6px;
    color: #e6edf3;
    font-size: 14px;
    outline: none;
}

.creator-search:focus {
    border-color: #5f875f;
}

.creator-search::placeholder {
    color: #8b949e;
}

.picker-item-number {
    font-size: 13px;
    color: #5f875f;
    font-weight: 600;
    margin-right: 8px;
    flex-shrink: 0;
}

.picker-item-branch {
    font-size: 11px;
    color: #808080;
    font-family: monospace;
    margin-top: 2px;
}

.picker-item-labels {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
    margin-top: 4px;
}

.picker-label {
    font-size: 11px;
    padding: 2px 6px;
    background-color: #30363d;
    border-radius: 4px;
    color: #8b949e;
}

.creator-textarea {
    width: 100%;
    min-height: 120px;
    padding: 12px;
    margin-bottom: 12px;
    background-color: #0d1117;
    border: 1px solid #30363d;
    border-radius: 6px;
    color: #e6edf3;
    font-size: 14px;
    font-family: inherit;
    resize: vertical;
    outline: none;
}

.creator-textarea:focus {
    border-color: #5f875f;
}

.creator-textarea::placeholder {
    color: #8b949e;
}

.creator-submit {
    width: 100%;
    padding: 10px;
    background-color: #5f875f;
    border: none;
    border-radius: 6px;
    color: #e6edf3;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.15s;
}

.creator-submit:hover:not(:disabled) {
    background-color: #87af87;
}

.creator-submit:disabled {
    opacity: 0.4;
    cursor: not-allowed;
}

.creator-loading {
    padding: 24px;
    text-align: center;
    color: #5f875f;
    font-size: 14px;
}

.creator-error {
    padding: 16px;
    text-align: center;
    color: #d78787;
    font-size: 13px;
}

.creator-hint {
    font-size: 12px;
    color: #8b949e;
    text-align: center;
    padding-top: 4px;
}

/* Split pane */
.split-pane-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    user-select: none;
}

.split-pane-top {
    overflow-y: auto;
    padding: 32px;
}

.split-pane-bottom {
    overflow-y: auto;
}

.split-pane-divider {
    height: 6px;
    background-color: #2a2a4a;
    cursor: row-resize;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background-color 0.15s;
}

.split-pane-divider:hover,
.split-pane-divider.dragging {
    background-color: #a78bfa;
}

.split-pane-divider-handle {
    width: 40px;
    height: 2px;
    background-color: #555;
    border-radius: 1px;
}

.split-pane-divider:hover .split-pane-divider-handle,
.split-pane-divider.dragging .split-pane-divider-handle {
    background-color: #e0e0e0;
}

/* Selected step card */
.step-card.selected {
    border-color: #a78bfa;
    box-shadow: 0 0 0 1px #a78bfa;
}

/* Terminal view */
.terminal-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    background-color: #0d1117;
}

.terminal-header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 16px;
    background-color: #161b22;
    border-bottom: 1px solid #21262d;
    flex-shrink: 0;
}

.terminal-step-name {
    font-size: 13px;
    font-weight: 600;
    color: #e0e0e0;
}

.terminal-attempt {
    font-size: 12px;
    color: #888;
}

.terminal-status-indicator {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
}

.terminal-status-text {
    font-size: 12px;
    font-weight: 500;
}

.terminal-status-text.pending { color: #888; }
.terminal-status-text.running { color: #60a5fa; }
.terminal-status-text.completed { color: #34d399; }
.terminal-status-text.failed { color: #f87171; }
.terminal-status-text.skipped { color: #666; }

.terminal-output {
    flex: 1;
    overflow-y: auto;
    padding: 12px 16px;
}

.terminal-output-text {
    font-family: "SF Mono", "Fira Code", "Cascadia Code", Menlo, Monaco, "Courier New", monospace;
    font-size: 13px;
    line-height: 1.5;
    color: #c9d1d9;
    white-space: pre-wrap;
    word-break: break-all;
    margin: 0;
}

.notification-bar {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    max-height: 120px;
    overflow-y: auto;
    background-color: #1c2128;
    border-top: 1px solid #30363d;
    padding: 8px 16px;
    z-index: 100;
}

.notification-item {
    padding: 4px 8px;
    margin-bottom: 4px;
    font-size: 12px;
    color: #f0883e;
    background-color: rgba(240, 136, 62, 0.1);
    border-left: 3px solid #f0883e;
    border-radius: 3px;
}
"#;
