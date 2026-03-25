pub(crate) const APP_CSS: &str = r#"
:root {
    /* Surface layers (darkest to highest) */
    --surface-0: #0c0e12;
    --surface-1: #131720;
    --surface-2: #1a1f2b;
    --surface-3: #242a38;
    --surface-4: #2e3546;

    /* Borders */
    --border-subtle: #1e2433;
    --border-default: #2a3142;
    --border-emphasis: #3d4759;

    /* Text */
    --text-primary: #e2e8f0;
    --text-secondary: #8892a4;
    --text-tertiary: #5a6478;

    /* Accent (sage green) */
    --accent: #6b9e6b;
    --accent-hover: #7db87d;
    --accent-subtle: rgba(107, 158, 107, 0.12);

    /* Semantic status */
    --status-running: #6b9e6b;
    --status-success: #87b887;
    --status-error: #c47070;
    --status-warning: #c4a04e;
    --status-pending: #5a6478;

    /* Typography */
    --font-sans: "Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    --font-mono: "JetBrains Mono", "SF Mono", "Fira Code", monospace;

    /* Spacing */
    --space-1: 4px;
    --space-2: 8px;
    --space-3: 12px;
    --space-4: 16px;
    --space-5: 20px;
    --space-6: 24px;
    --space-8: 32px;

    /* Radius */
    --radius-sm: 6px;
    --radius-md: 8px;
    --radius-lg: 12px;

    /* Transitions */
    --transition-fast: 100ms ease-out;
    --transition-normal: 150ms ease-out;
    --transition-slow: 200ms ease-out;
}

* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: var(--font-sans);
    font-size: 13px;
    line-height: 1.5;
    color: var(--text-primary);
    background: var(--surface-0);
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
    background-color: var(--surface-0);
    border-bottom: 1px solid var(--border-default);
    min-height: 40px;
    padding: 0 var(--space-1);
    overflow-x: auto;
    flex-shrink: 0;
}

.tab {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    cursor: pointer;
    border: 1px solid transparent;
    border-bottom: none;
    border-radius: var(--radius-sm) var(--radius-sm) 0 0;
    margin-right: 2px;
    font-size: 13px;
    color: var(--text-secondary);
    transition: background-color var(--transition-normal), color var(--transition-normal);
    white-space: nowrap;
    max-width: 200px;
}

.tab:hover {
    background-color: var(--surface-2);
    color: var(--text-primary);
}

.tab.active {
    background-color: var(--surface-1);
    border-color: var(--accent);
    color: var(--text-primary);
}

.tab-name {
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
}

.tab-session-id {
    font-size: 10px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 80px;
}

.tab-close {
    background: none;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 12px;
    padding: 0 2px;
    line-height: 1;
    border-radius: 3px;
}

.tab-close:hover {
    background-color: var(--border-default);
    color: var(--text-primary);
}

.tab-new {
    background: none;
    border: 1px solid var(--border-default);
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 16px;
    padding: var(--space-1) 10px;
    margin-left: var(--space-1);
    border-radius: var(--radius-sm);
    transition: background-color var(--transition-normal), color var(--transition-normal);
}

.tab-new:hover {
    background-color: var(--surface-2);
    color: var(--accent);
}

/* Status indicators */
.status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
}

.status-dot.idle {
    background-color: var(--status-pending);
}

.status-dot.pending {
    background-color: var(--status-pending);
}

.status-dot.running {
    background-color: var(--status-running);
    animation: pulse 1.5s ease-in-out infinite;
}

.status-dot.completed {
    background-color: var(--status-success);
}

.status-dot.failed {
    background-color: var(--status-error);
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
    color: var(--text-secondary);
    font-size: 16px;
}

/* Workflow view */
.workflow-header {
    margin-bottom: var(--space-6);
}

.workflow-header h2 {
    font-size: 20px;
    font-weight: 600;
    color: var(--text-primary);
}

.workflow-header p {
    font-size: 14px;
    color: var(--text-secondary);
    margin-top: var(--space-1);
}

.workflow-meta {
    display: flex;
    gap: var(--space-3);
    margin-top: var(--space-3);
    flex-wrap: wrap;
}

.meta-tag {
    font-size: 11px;
    padding: var(--space-1) 10px;
    background-color: var(--surface-2);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
}

.steps-section h3 {
    font-size: 16px;
    font-weight: 500;
    color: var(--text-primary);
    margin-bottom: var(--space-3);
}

.steps-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
}

/* Step card */
.step-card {
    background-color: var(--surface-2);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    padding: var(--space-4);
    transition: border-color var(--transition-normal);
    cursor: pointer;
}

.step-card:hover {
    border-color: var(--accent);
}

.step-card.running {
    border-color: var(--status-running);
}

.step-card.completed {
    border-color: var(--status-success);
}

.step-card.failed {
    border-color: var(--status-error);
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
    color: var(--text-primary);
    flex: 1;
}

.step-status-text {
    font-size: 11px;
    font-weight: 500;
}

.step-status-text.pending { color: var(--status-pending); }
.step-status-text.running { color: var(--status-running); }
.step-status-text.completed { color: var(--status-success); }
.step-status-text.failed { color: var(--status-error); }
.step-status-text.skipped { color: var(--status-pending); }

.step-details {
    margin-top: var(--space-2);
    font-size: 11px;
    color: var(--text-secondary);
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
}

.step-detail-label {
    color: var(--text-tertiary);
}

.step-deps {
    margin-top: var(--space-2);
    font-size: 11px;
    color: var(--text-tertiary);
}

/* Workflow picker modal */
.picker-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(12, 14, 18, 0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
}

.picker-modal {
    background-color: var(--surface-2);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-lg);
    padding: var(--space-6);
    min-width: 360px;
    max-width: 480px;
    max-height: 60vh;
    display: flex;
    flex-direction: column;
}

.picker-modal h3 {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: var(--space-4);
}

.picker-list {
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
}

.picker-item {
    padding: var(--space-3);
    border-radius: var(--radius-md);
    cursor: pointer;
    border: 1px solid transparent;
    transition: background-color var(--transition-normal), border-color var(--transition-normal);
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
}

.picker-item:hover {
    background-color: var(--surface-3);
    border-color: var(--accent);
}

.picker-item-name {
    font-size: 14px;
    font-weight: 500;
    color: var(--text-primary);
}

.picker-item-desc {
    font-size: 11px;
    color: var(--text-secondary);
}

.picker-empty {
    padding: var(--space-6);
    text-align: center;
    color: var(--text-secondary);
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
    margin-bottom: var(--space-3);
}

.mode-btn {
    background-color: var(--surface-0);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    padding: var(--space-4) var(--space-3);
    cursor: pointer;
    text-align: left;
    color: var(--text-primary);
    transition: background-color var(--transition-normal), border-color var(--transition-normal);
}

.mode-btn:hover:not(:disabled) {
    background-color: var(--surface-3);
    border-color: var(--accent);
}

.mode-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
}

.mode-btn-title {
    font-size: 14px;
    font-weight: 600;
    margin-bottom: var(--space-1);
}

.mode-btn-desc {
    font-size: 11px;
    color: var(--text-secondary);
}

.creator-header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-bottom: var(--space-4);
}

.creator-header h3 {
    margin-bottom: 0;
}

.creator-back {
    background: none;
    border: 1px solid var(--border-default);
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 12px;
    padding: var(--space-1) 10px;
    border-radius: var(--radius-sm);
    transition: background-color var(--transition-normal), color var(--transition-normal);
}

.creator-back:hover {
    background-color: var(--surface-3);
    color: var(--text-primary);
}

.creator-search {
    width: 100%;
    padding: 10px var(--space-3);
    margin-bottom: var(--space-3);
    background-color: var(--surface-0);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-size: 14px;
    outline: none;
}

.creator-search:focus {
    border-color: var(--accent);
}

.creator-search::placeholder {
    color: var(--text-secondary);
}

.picker-item-number {
    font-size: 13px;
    color: var(--accent);
    font-weight: 600;
    margin-right: var(--space-2);
    flex-shrink: 0;
}

.picker-item-branch {
    font-size: 11px;
    color: var(--text-tertiary);
    font-family: var(--font-mono);
    margin-top: 2px;
}

.picker-item-labels {
    display: flex;
    gap: var(--space-1);
    flex-wrap: wrap;
    margin-top: var(--space-1);
}

.picker-label {
    font-size: 11px;
    padding: 2px var(--space-2);
    background-color: var(--surface-3);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
}

.creator-textarea {
    width: 100%;
    min-height: 120px;
    padding: var(--space-3);
    margin-bottom: var(--space-3);
    background-color: var(--surface-0);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-size: 14px;
    font-family: inherit;
    resize: vertical;
    outline: none;
}

.creator-textarea:focus {
    border-color: var(--accent);
}

.creator-textarea::placeholder {
    color: var(--text-secondary);
}

.creator-submit {
    width: 100%;
    padding: 10px;
    background-color: var(--accent);
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: background-color var(--transition-normal);
}

.creator-submit:hover:not(:disabled) {
    background-color: var(--accent-hover);
}

.creator-submit:disabled {
    opacity: 0.4;
    cursor: not-allowed;
}

.creator-loading {
    padding: var(--space-6);
    text-align: center;
    color: var(--accent);
    font-size: 14px;
}

.creator-error {
    padding: var(--space-4);
    text-align: center;
    color: var(--status-error);
    font-size: 13px;
}

.creator-hint {
    font-size: 11px;
    color: var(--text-secondary);
    text-align: center;
    padding-top: var(--space-1);
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
    padding: var(--space-8);
}

.split-pane-bottom {
    overflow: hidden;
    width: 100%;
}

.split-pane-divider {
    height: 6px;
    background-color: var(--border-default);
    cursor: row-resize;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background-color var(--transition-normal);
}

.split-pane-divider:hover,
.split-pane-divider.dragging {
    background-color: var(--accent);
}

.split-pane-divider-handle {
    width: 40px;
    height: 2px;
    background-color: var(--border-emphasis);
    border-radius: 1px;
}

.split-pane-divider:hover .split-pane-divider-handle,
.split-pane-divider.dragging .split-pane-divider-handle {
    background-color: var(--text-primary);
}

/* Selected step card */
.step-card.selected {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
}

/* Terminal view */
.terminal-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    background-color: var(--surface-0);
    overflow: hidden;
}

.terminal-header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-4);
    background-color: var(--surface-1);
    border-bottom: 1px solid var(--border-subtle);
    flex-shrink: 0;
}

.terminal-step-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
}

.terminal-attempt {
    font-size: 11px;
    color: var(--text-secondary);
}

.terminal-status-indicator {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-left: auto;
}

.terminal-status-text {
    font-size: 11px;
    font-weight: 500;
}

.terminal-status-text.pending { color: var(--status-pending); }
.terminal-status-text.running { color: var(--status-running); }
.terminal-status-text.completed { color: var(--status-success); }
.terminal-status-text.failed { color: var(--status-error); }
.terminal-status-text.skipped { color: var(--status-pending); }

.terminal-xterm-container {
    flex: 1;
    position: relative;
    overflow: hidden;
    min-height: 0;
    background-color: var(--surface-0);
}

.terminal-xterm-container .xterm {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
}

.terminal-xterm-container .xterm-viewport {
    overflow-y: auto !important;
}

.notification-bar {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    max-height: 120px;
    overflow-y: auto;
    background-color: var(--surface-2);
    border-top: 1px solid var(--border-default);
    padding: var(--space-2) var(--space-4);
    z-index: 100;
}

.notification-item {
    padding: var(--space-1) var(--space-2);
    margin-bottom: var(--space-1);
    font-size: 11px;
    color: var(--status-warning);
    background-color: rgba(196, 160, 78, 0.1);
    border-left: 3px solid var(--status-warning);
    border-radius: 3px;
}

/* Home tab */
.home-tab {
    font-weight: 600;
}

/* Dashboard */
.dashboard {
    padding: var(--space-8);
    overflow-y: auto;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
}

.dashboard-loading,
.dashboard-error {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    font-size: 14px;
}

.dashboard-loading {
    color: var(--text-secondary);
}

.dashboard-error {
    color: var(--status-error);
}

.dashboard-section {
    background-color: var(--surface-1);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    padding: var(--space-5);
}

.dashboard-section-header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-bottom: var(--space-4);
}

.dashboard-section-header h3 {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
}

.dashboard-count {
    font-size: 11px;
    font-weight: 600;
    padding: 2px var(--space-2);
    background-color: var(--surface-3);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
}

.dashboard-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
}

.dashboard-row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-sm);
    transition: background-color var(--transition-normal);
}

.dashboard-row:hover {
    background-color: var(--surface-2);
}

.dashboard-number {
    font-size: 13px;
    color: var(--accent);
    font-weight: 600;
    flex-shrink: 0;
    min-width: 40px;
}

.dashboard-title {
    font-size: 13px;
    color: var(--text-primary);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.dashboard-branch {
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--text-tertiary);
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex-shrink: 0;
}

.dashboard-time {
    font-size: 11px;
    color: var(--text-tertiary);
    flex-shrink: 0;
    min-width: 50px;
    text-align: right;
}

.dashboard-labels {
    display: flex;
    gap: var(--space-1);
    flex-shrink: 0;
}

.dashboard-label {
    font-size: 11px;
    padding: 1px var(--space-2);
    background-color: var(--accent-subtle);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
}

.ci-badge {
    font-size: 10px;
    font-weight: 600;
    padding: 1px var(--space-2);
    border-radius: var(--radius-sm);
    flex-shrink: 0;
}

.ci-badge.passed {
    background-color: rgba(135, 184, 135, 0.15);
    color: var(--status-success);
}

.ci-badge.failed {
    background-color: rgba(196, 112, 112, 0.15);
    color: var(--status-error);
}

.ci-badge.pending {
    background-color: rgba(196, 160, 78, 0.15);
    color: var(--status-warning);
}

.ci-badge.none {
    display: none;
}

.dashboard-ready-btn {
    font-size: 11px;
    font-weight: 600;
    padding: 2px var(--space-3);
    background-color: var(--accent-subtle);
    border: 1px solid var(--accent);
    border-radius: var(--radius-sm);
    color: var(--accent-hover);
    cursor: pointer;
    flex-shrink: 0;
    transition: background-color var(--transition-normal), color var(--transition-normal);
}

.dashboard-ready-btn:hover:not(:disabled) {
    background-color: var(--accent);
    color: var(--text-primary);
}

.dashboard-ready-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
}

.dashboard-empty {
    padding: var(--space-4);
    text-align: center;
    font-size: 13px;
    color: var(--text-tertiary);
}

/* Quick actions strip */
.quick-actions {
    display: flex;
    gap: var(--space-3);
    flex-wrap: wrap;
}

.quick-action-btn {
    padding: var(--space-2) var(--space-4);
    background-color: var(--surface-2);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background-color var(--transition-normal), border-color var(--transition-normal);
}

.quick-action-btn:hover:not(:disabled) {
    background-color: var(--surface-3);
    border-color: var(--accent);
}

.quick-action-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
}

.quick-action-btn.primary {
    background-color: var(--accent-subtle);
    border-color: var(--accent);
    color: var(--accent-hover);
}

.quick-action-btn.primary:hover:not(:disabled) {
    background-color: var(--accent);
    color: var(--text-primary);
}

.quick-action-error {
    color: var(--status-error);
    font-size: 12px;
}

/* Backlog tab */
.backlog-tab {
    padding: var(--space-8);
    overflow-y: auto;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
}

.backlog-input-form {
    flex-shrink: 0;
}

.backlog-input {
    width: 100%;
    padding: 10px var(--space-3);
    background-color: var(--surface-1);
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-size: 14px;
    outline: none;
}

.backlog-input:focus {
    border-color: var(--accent);
}

.backlog-input::placeholder {
    color: var(--text-secondary);
}

.backlog-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
}

.backlog-item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-sm);
    transition: background-color var(--transition-normal);
}

.backlog-item:hover {
    background-color: var(--surface-2);
}

.backlog-item.completed .backlog-text {
    text-decoration: line-through;
    color: var(--text-tertiary);
}

.backlog-checkbox {
    flex-shrink: 0;
    cursor: pointer;
    accent-color: var(--accent);
}

.backlog-priority-urgent {
    font-size: 12px;
    font-weight: 700;
    color: var(--status-error);
    flex-shrink: 0;
}

.backlog-priority-important {
    font-size: 12px;
    font-weight: 700;
    color: var(--status-warning);
    flex-shrink: 0;
}

.backlog-text {
    font-size: 13px;
    color: var(--text-primary);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.backlog-tag {
    font-size: 11px;
    padding: 1px var(--space-2);
    background-color: var(--accent-subtle);
    border-radius: var(--radius-sm);
    color: var(--accent-hover);
    flex-shrink: 0;
}

.backlog-context {
    font-size: 11px;
    padding: 1px var(--space-2);
    background-color: var(--surface-3);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    flex-shrink: 0;
}

.backlog-time {
    font-size: 11px;
    color: var(--text-tertiary);
    flex-shrink: 0;
    min-width: 50px;
    text-align: right;
}

.backlog-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 1;
    font-size: 14px;
    color: var(--text-secondary);
}
"#;
