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
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
}

/* Tab bar */
.tab-bar {
    display: flex;
    align-items: center;
    background-color: #16162a;
    border-bottom: 1px solid #2a2a4a;
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
    color: #888;
    transition: background-color 0.15s, color 0.15s;
    white-space: nowrap;
    max-width: 200px;
}

.tab:hover {
    background-color: #1e1e3a;
    color: #ccc;
}

.tab.active {
    background-color: #1a1a2e;
    border-color: #2a2a4a;
    color: #e0e0e0;
}

.tab-name {
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
}

.tab-close {
    background: none;
    border: none;
    color: #555;
    cursor: pointer;
    font-size: 12px;
    padding: 0 2px;
    line-height: 1;
    border-radius: 3px;
}

.tab-close:hover {
    background-color: #333;
    color: #e0e0e0;
}

.tab-new {
    background: none;
    border: 1px solid #2a2a4a;
    color: #888;
    cursor: pointer;
    font-size: 16px;
    padding: 4px 10px;
    margin-left: 4px;
    border-radius: 6px;
    transition: background-color 0.15s, color 0.15s;
}

.tab-new:hover {
    background-color: #1e1e3a;
    color: #a78bfa;
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

.status-dot.pending {
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
    cursor: pointer;
}

.step-card:hover {
    border-color: #3a3a5a;
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

/* Workflow picker modal */
.picker-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
}

.picker-modal {
    background-color: #1e1e3a;
    border: 1px solid #2a2a4a;
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
    color: #e0e0e0;
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
    background-color: #2a2a4a;
    border-color: #a78bfa;
}

.picker-item-name {
    font-size: 14px;
    font-weight: 500;
    color: #e0e0e0;
}

.picker-item-desc {
    font-size: 12px;
    color: #888;
}

.picker-empty {
    padding: 24px;
    text-align: center;
    color: #555;
    font-size: 14px;
}
"#;
