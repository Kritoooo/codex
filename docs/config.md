# Configuration

For basic configuration instructions, see [this documentation](https://developers.openai.com/codex/config-basic).

For advanced configuration instructions, see [this documentation](https://developers.openai.com/codex/config-advanced).

For a full configuration reference, see [this documentation](https://developers.openai.com/codex/config-reference).

## Connecting to MCP servers

Codex can connect to MCP servers configured in `~/.codex/config.toml`. See the configuration reference for the latest MCP server options:

- https://developers.openai.com/codex/config-reference

## Notify

Codex can run a notification hook when the agent finishes a turn. See the configuration reference for the latest notification settings:

- https://developers.openai.com/codex/config-reference

## JSON Schema

The generated JSON Schema for `config.toml` lives at `codex-rs/core/config.schema.json`.

## Notices

Codex stores "do not show again" flags for some UI prompts under the `[notice]` table.

Ctrl+C/Ctrl+D quitting uses a ~1 second double-press hint (`ctrl + c again to quit`).

## TUI status line

Codex can run a custom status line command in the TUI footer. Configure it in `~/.codex/config.toml`:

```toml
[tui.status_line]
command = ["/path/to/statusline.sh"]
show_hints = true
update_interval_ms = 300
timeout_ms = 1000
```

`command` can be a shell-like string or an argv array. The command is executed
directly (no shell), so `~` and `$HOME` are not expanded unless you wrap it.
If you need shell features, wrap the command yourself, for example:

```toml
command = ["bash", "-lc", "~/.codex/statusline.sh"]
```

The command receives a JSON object on stdin describing the current session
(`model`, `model_provider`, `cwd`, `git_branch`, `task_running`, `review_mode`,
`context_window_percent`, `context_window_used_tokens`, `token_usage`), and should
print a single line to stdout (ANSI colors supported).

By default, footer hints are appended to the status line so the footer stays
single-line when a status line command is enabled. Set `show_hints = false` to
hide the hints when the status line is active.
