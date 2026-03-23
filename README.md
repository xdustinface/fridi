<div align="center">

<img src="assets/logo.svg" width="80" height="80" alt="conductor logo">

# conductor

![Pre-commit](https://github.com/xdustinface/conductor/actions/workflows/pre-commit.yml/badge.svg) ![Tests](https://github.com/xdustinface/conductor/actions/workflows/test.yml/badge.svg) [![codecov](https://codecov.io/gh/xdustinface/conductor/graph/badge.svg)](https://codecov.io/gh/xdustinface/conductor) ![License](https://img.shields.io/badge/license-MIT-blue.svg)

</div>

## What is conductor?

Conductor is an AI workflow orchestrator built in Rust. It lets you define
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

```
conductor/
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
git clone https://github.com/xdustinface/conductor.git
cd conductor
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
cargo run -p conductor-ui
```

## Workflow Example

Below is a simplified workflow that monitors pull requests, reviews them, applies
fixes, and watches CI:

```yaml
name: pr-babysitter
description: Monitor PRs, fix issues, watch CI until green

config:
  repo: "owner/repo"

triggers:
  - type: cron
    schedule: "*/30 * * * *"
  - type: manual

steps:
  - name: check-prs
    agent: claude
    skill: recap
    outputs: [prs_needing_attention]

  - name: review
    agent: claude
    skill: code-review
    depends_on: [check-prs]
    condition: "steps.check-prs.outputs.prs_needing_attention > 0"
    on_failure: notify

  - name: fix
    agent: claude
    skill: fixup
    depends_on: [review]

  - name: watch-ci
    agent: claude
    skill: watch
    depends_on: [fix]
    retry:
      max_attempts: 10
      interval: "5m"
    on_failure: notify
```

## License

This project is licensed under the [MIT License](LICENSE).
