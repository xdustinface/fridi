pub(crate) const APP_CSS: &str = r#"
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    background-color: #1a1a2e;
    color: #e0e0e0;
    line-height: 1.5;
}

.app-layout {
    display: flex;
    height: 100vh;
    overflow: hidden;
}

/* Sidebar */
.sidebar {
    width: 260px;
    min-width: 260px;
    background-color: #16162a;
    border-right: 1px solid #2a2a4a;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

.sidebar-header {
    padding: 20px;
    border-bottom: 1px solid #2a2a4a;
}

.sidebar-header h1 {
    font-size: 20px;
    font-weight: 600;
    color: #a78bfa;
    letter-spacing: 0.5px;
}

.sidebar-header p {
    font-size: 11px;
    color: #666;
    margin-top: 4px;
}

.workflow-list {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
}

.workflow-item {
    padding: 12px;
    margin-bottom: 4px;
    border-radius: 8px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background-color 0.15s, border-color 0.15s;
}

.workflow-item:hover {
    background-color: #1e1e3a;
}

.workflow-item.selected {
    background-color: #1e1e3a;
    border-color: #a78bfa;
}

.workflow-item-header {
    display: flex;
    align-items: center;
    gap: 8px;
}

.workflow-item-name {
    font-size: 14px;
    font-weight: 500;
    color: #e0e0e0;
    flex: 1;
}

.workflow-item-desc {
    font-size: 12px;
    color: #888;
    margin-top: 4px;
}

/* Status indicators */
.status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
}

.status-dot.idle {
    background-color: #555;
}

.status-dot.running {
    background-color: #60a5fa;
    animation: pulse 1.5s ease-in-out infinite;
}

.status-dot.completed {
    background-color: #34d399;
}

.status-dot.failed {
    background-color: #f87171;
}

@keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
}

/* Main content */
.main-content {
    flex: 1;
    overflow-y: auto;
    padding: 32px;
}

.empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #555;
    font-size: 16px;
}

/* Workflow view */
.workflow-header {
    margin-bottom: 24px;
}

.workflow-header h2 {
    font-size: 24px;
    font-weight: 600;
    color: #e0e0e0;
}

.workflow-header p {
    font-size: 14px;
    color: #888;
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
    background-color: #1e1e3a;
    border: 1px solid #2a2a4a;
    border-radius: 4px;
    color: #aaa;
}

.workflow-actions {
    margin-bottom: 24px;
}

.btn-run {
    padding: 8px 20px;
    font-size: 14px;
    font-weight: 500;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    background-color: #a78bfa;
    color: #1a1a2e;
    transition: background-color 0.15s;
}

.btn-run:hover {
    background-color: #8b5cf6;
}

.btn-run:disabled {
    background-color: #444;
    color: #888;
    cursor: not-allowed;
}

.steps-section h3 {
    font-size: 16px;
    font-weight: 500;
    color: #ccc;
    margin-bottom: 12px;
}

.steps-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
}

/* Step card */
.step-card {
    background-color: #1e1e3a;
    border: 1px solid #2a2a4a;
    border-radius: 8px;
    padding: 16px;
    transition: border-color 0.15s;
}

.step-card.running {
    border-color: #60a5fa;
}

.step-card.completed {
    border-color: #34d399;
}

.step-card.failed {
    border-color: #f87171;
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
    color: #e0e0e0;
    flex: 1;
}

.step-status-text {
    font-size: 12px;
    font-weight: 500;
}

.step-status-text.pending { color: #888; }
.step-status-text.running { color: #60a5fa; }
.step-status-text.completed { color: #34d399; }
.step-status-text.failed { color: #f87171; }
.step-status-text.skipped { color: #666; }

.step-details {
    margin-top: 8px;
    font-size: 12px;
    color: #888;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
}

.step-detail-label {
    color: #666;
}

.step-deps {
    margin-top: 6px;
    font-size: 11px;
    color: #666;
}

.step-hint {
    margin-top: 6px;
    font-size: 11px;
    color: #555;
    font-style: italic;
}
"#;
