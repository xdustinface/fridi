# Writing Custom Workflows

Fridi workflows are YAML files that describe a directed acyclic graph (DAG) of
steps.  The engine resolves dependencies, evaluates conditions, spawns agents,
retries on failure, and emits events for the UI and notification system.

## YAML Schema Reference

### Top-level fields

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | Unique workflow identifier. Must be non-empty. |
| `description` | string | no | Human-readable description. |
| `config` | object | no | Workflow-level configuration passed to every step. |
| `triggers` | list | no | When the workflow should run. |
| `notifications` | object | no | Where to send alerts on failure or completion. |
| `steps` | list | yes | Ordered list of steps. Must contain at least one. |

### `config`

| Field | Type | Description |
|---|---|---|
| `repo` | string | GitHub repository in `owner/repo` format. |
| *(any key)* | any | Arbitrary key-value pairs available to steps via context. |

Environment variable interpolation is supported with `${VAR_NAME}` syntax.

### `triggers[]`

Each trigger has a `type` field.

| Type | Extra fields | Description |
|---|---|---|
| `cron` | `schedule` (cron expression) | Run on a schedule. |
| `manual` | *(none)* | Run on demand. |

### `notifications`

| Field | Type | Description |
|---|---|---|
| `slack.webhook_url` | string | Slack incoming-webhook URL. |
| `slack.channel` | string | Slack channel override (optional). |
| `telegram.bot_token` | string | Telegram bot API token. |
| `telegram.chat_id` | string | Telegram chat ID to post to. |

### `steps[]`

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | string | *(required)* | Unique step name within the workflow. |
| `agent` | string | `null` | Agent type to run the step (e.g. `claude`). |
| `skill` | string | `null` | Skill/capability the agent should use. |
| `args` | string | `null` | Extra arguments passed to the agent. |
| `prompt` | string | `null` | Prompt text sent to the agent. Supports `${ENV}` and `{{template}}` interpolation. |
| `depends_on` | list | `[]` | Step names that must complete before this step runs. |
| `condition` | string | `null` | Expression that must be truthy for the step to run. Supports `steps.<name>` references. |
| `for_each` | string | `null` | Expression yielding a list; the step runs once per item. |
| `outputs` | list | `[]` | Named outputs this step produces, available to downstream steps. |
| `on_failure` | enum | `stop` | What to do when the step fails: `stop`, `notify`, or `continue`. |
| `retry` | object | `null` | Retry configuration (see below). |
| `type` | enum | `null` | Step type override. Use `notification` for notification-only steps. |
| `message` | string | `null` | Message template for notification steps. Supports `{{template}}` interpolation. |

### `retry`

| Field | Type | Default | Description |
|---|---|---|---|
| `max_attempts` | integer | `1` | Maximum number of attempts. |
| `interval` | string | `"1m"` | Duration between retries (`5s`, `10m`, `1h`). |
| `until` | string | `null` | Condition expression that stops retrying when truthy. |

### Conditions

Condition strings are evaluated as follows:

- `"true"` / `"false"` -- literal boolean.
- `"steps.<step_name>"` -- truthy if the referenced step produced non-empty output.
- Any other string -- treated as truthy (reserved for future expression support).

### Failure policies

| Policy | Behavior |
|---|---|
| `stop` | Abort the entire workflow. |
| `notify` | Send a notification and mark the step as failed, but continue downstream steps. |
| `continue` | Silently mark the step as failed and continue. |

## Example Walkthrough: pr-babysitter

The `pr-babysitter.yaml` workflow demonstrates all major features.

### Step 1: check-prs

```yaml
- name: check-prs
  agent: claude
  skill: recap
  prompt: "Check for open PRs that need attention on ${FRIDI_REPO}..."
  outputs: [prs_needing_attention]
```

This step has no dependencies, so it runs first.  It uses the `recap` skill to
scan the repository and produces a `prs_needing_attention` output list.

### Step 2: review

```yaml
- name: review
  agent: claude
  skill: code-review
  depends_on: [check-prs]
  condition: "steps.check-prs"
  for_each: "steps.check-prs.outputs.prs_needing_attention"
  prompt: "Review PR #{{item.number}} on ${FRIDI_REPO}"
  on_failure: notify
  outputs: [review_findings]
```

Waits for `check-prs` to complete. The `condition` ensures this step is skipped
if no PRs need attention. The `for_each` iterates over each PR, spawning a
review agent per item. If any review fails, a notification is sent but the
workflow continues.

### Step 3: fix

```yaml
- name: fix
  agent: claude
  skill: fixup
  depends_on: [review]
  condition: "steps.review"
  for_each: "steps.review.outputs.prs_with_findings"
  prompt: "Fix issues found in PR #{{item.number}} on ${FRIDI_REPO}"
  on_failure: notify
```

Only runs if the review step produced findings. Iterates over PRs that have
actionable issues and applies fixes.

### Step 4: watch-ci

```yaml
- name: watch-ci
  agent: claude
  skill: watch
  depends_on: [fix]
  for_each: "steps.fix.outputs.fixed_prs"
  prompt: "Watch CI on PR #{{item.number}} on ${FRIDI_REPO} until green"
  retry:
    max_attempts: 10
    interval: "5m"
    until: "ci_status == green"
  on_failure: notify
```

Polls CI status for each fixed PR.  Retries up to 10 times at 5-minute
intervals.  If CI is still failing after all attempts, a notification is sent.

### Step 5: notify-complete

```yaml
- name: notify-complete
  type: notification
  depends_on: [watch-ci]
  message: "PR babysitter completed. {{steps.watch-ci.summary}}"
```

A pure notification step (no agent). It fires after all CI watching is done and
sends a summary to the configured notification channels.

## Tips

- Keep step names short and descriptive -- they appear in the UI and logs.
- Use `on_failure: notify` for steps where you want visibility but not a hard
  stop.
- Use `on_failure: stop` (the default) for critical steps where continuing would
  be pointless.
- Put environment-specific values in `${ENV_VAR}` references so the same
  workflow file works across environments.
- The `for_each` field is a string expression, not evaluated at parse time. The
  engine resolves it at runtime from the context.
