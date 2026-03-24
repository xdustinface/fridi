<div align="center">

<img src="assets/logo.svg" width="80" height="80" alt="fridi logo">

# fridi

![Pre-commit](https://github.com/xdustinface/fridi/actions/workflows/pre-commit.yml/badge.svg) ![Tests](https://github.com/xdustinface/fridi/actions/workflows/test.yml/badge.svg) [![codecov](https://codecov.io/gh/xdustinface/fridi/graph/badge.svg)](https://codecov.io/gh/xdustinface/fridi) ![License](https://img.shields.io/badge/license-MIT-blue.svg)

</div>

## What is fridi?

Fridi is an AI workflow orchestrator built in Rust. It lets you define
workflows as YAML-based DAGs, spawn AI coding agents (starting with the Claude
CLI), observe them through a live desktop UI, and receive notifications when
things need attention.

## Key Features

- **YAML workflow definitions with DAG execution** -- declare steps, dependencies,
  conditions, retries, and failure policies in plain YAML.
- **Model-agnostic agent trait** -- ships with a Claude CLI integration and is
  designed to support additional agent backends.
- **PTY-based agent execution** -- agents run in real pseudo-terminals, preserving
  full terminal fidelity for interactive tools.
- **Dioxus desktop UI** -- a multi-window, tab-per-session interface with live
  step progress and a split-pane layout.
- **Session persistence and recovery** -- workflow state is saved so runs can
  survive restarts.
- **Pluggable notifications** -- Slack and Telegram webhooks out of the box, with
  a simple trait for adding more.
- **Cron and manual workflow triggers** -- schedule recurring runs or kick them
  off on demand.

## Architecture

```text
fridi/
+-- crates/
|   +-- core/       # Workflow schema, YAML parsing, DAG, execution engine, sessions
|   +-- agent/      # Agent trait, PTY spawning, Claude CLI implementation
|   +-- notify/     # Notification trait, Telegram & Slack webhooks
|   +-- trigger/    # Trigger trait, cron scheduler, manual trigger
|   +-- ui/         # Dioxus desktop app (multi-window, tabs, split pane)
+-- workflows/      # YAML workflow definitions
+-- sessions/       # Persisted session data
```

## Getting Started

### Prerequisites

- **Rust** (stable) -- install via [rustup](https://rustup.rs)
- **Python 3.12+** -- for pre-commit hooks
- **Claude CLI** -- required to run Claude-based agent steps

### Build

```sh
git clone https://github.com/xdustinface/fridi.git
cd fridi
cargo build --workspace
```

### Install pre-commit hooks

```sh
pip install pre-commit
pre-commit install
```

### Run tests

```sh
cargo test --workspace
```

### Run the UI

```sh
cargo run -p fridi-ui
```

## PR Babysitter Workflow

The **pr-babysitter** workflow is the flagship example. It continuously monitors
a repository's pull requests, reviews them, applies fixes, and watches CI until
everything is green -- or notifies you when it gets stuck.

### What it does

1. **check-prs** -- scans open PRs for failing CI, pending reviews, or
   unresolved comments.
2. **review** -- runs a code review on each PR that needs attention, producing
   structured findings.
3. **fix** -- applies fixes for the issues found during review.
4. **watch-ci** -- monitors CI on each fixed PR, retrying up to 10 times at
   5-minute intervals until the status is green.
5. **notify-complete** -- sends a summary notification when the run finishes.

### Configuration

Set these environment variables before running the workflow:

| Variable | Required | Description |
|---|---|---|
| `FRIDI_REPO` | yes | GitHub repository in `owner/repo` format |
| `FRIDI_SLACK_WEBHOOK_URL` | no | Slack incoming-webhook URL |
| `FRIDI_SLACK_CHANNEL` | no | Slack channel (e.g. `#dev-alerts`) |
| `FRIDI_TELEGRAM_BOT_TOKEN` | no | Telegram bot token |
| `FRIDI_TELEGRAM_CHAT_ID` | no | Telegram chat ID |

At least one notification channel (Slack or Telegram) is recommended so you
receive alerts when a step fails or when the workflow completes.

### Running the workflow

**Manual run:**

```sh
export FRIDI_REPO="owner/repo"
cargo run -p fridi-ui          # then select pr-babysitter from the workflow list
```

**Cron (automatic):** the workflow includes a cron trigger that fires every 30
minutes. When running fridi as a daemon, this trigger is evaluated automatically.

### Example output

```text
[check-prs]      Found 2 PRs needing attention: #41, #55
[review]         PR #41: 1 error (missing error handling in parser.rs:88)
[review]         PR #55: clean, no findings
[fix]            PR #41: applied fix, committed abc1234
[watch-ci]       PR #41: CI green after 1 attempt
[notify-complete] PR babysitter completed. 2 PRs processed, 1 fixed, all CI green.
```

### Workflow definition

See [`workflows/pr-babysitter.yaml`](workflows/pr-babysitter.yaml) for the full
YAML definition.  For details on writing custom workflows, see
[`workflows/README.md`](workflows/README.md).

## License

This project is licensed under the [MIT License](LICENSE).
