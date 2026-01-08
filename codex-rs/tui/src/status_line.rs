use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use codex_core::config::StatusLine;
use codex_core::git_info::current_branch_name;
use codex_core::protocol::TokenUsageInfo;
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::warn;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

#[derive(Debug)]
pub(crate) enum StatusLineUpdate {
    Updated(Option<String>),
    Failed,
}

#[derive(Clone, Debug)]
pub(crate) struct StatusLineRequest {
    pub(crate) model: String,
    pub(crate) model_provider: String,
    pub(crate) cwd: PathBuf,
    pub(crate) task_running: bool,
    pub(crate) review_mode: bool,
    pub(crate) context_window_percent: Option<i64>,
    pub(crate) context_window_used_tokens: Option<i64>,
    pub(crate) token_usage: Option<TokenUsageInfo>,
}

pub(crate) struct StatusLineManager {
    command: Vec<String>,
    update_interval: Duration,
    timeout: Duration,
    last_started_at: Option<Instant>,
    in_flight: bool,
}

impl StatusLineManager {
    pub(crate) fn new(config: StatusLine) -> Option<Self> {
        if config.command.is_empty() {
            return None;
        }

        Some(Self {
            command: config.command,
            update_interval: Duration::from_millis(config.update_interval_ms),
            timeout: Duration::from_millis(config.timeout_ms),
            last_started_at: None,
            in_flight: false,
        })
    }

    pub(crate) fn maybe_request(&mut self, request: StatusLineRequest, tx: AppEventSender) {
        if self.in_flight {
            return;
        }
        let now = Instant::now();
        if let Some(last) = self.last_started_at
            && now.duration_since(last) < self.update_interval
        {
            return;
        }

        self.in_flight = true;
        self.last_started_at = Some(now);
        let command = self.command.clone();
        let timeout = self.timeout;
        tokio::spawn(async move {
            let update = run_status_line_command(command, request, timeout).await;
            tx.send(AppEvent::StatusLineUpdated(update));
        });
    }

    pub(crate) fn mark_complete(&mut self) {
        self.in_flight = false;
    }
}

#[derive(Serialize)]
struct StatusLineInput {
    model: String,
    model_provider: String,
    cwd: String,
    git_branch: Option<String>,
    task_running: bool,
    review_mode: bool,
    context_window_percent: Option<i64>,
    context_window_used_tokens: Option<i64>,
    token_usage: Option<TokenUsageInfo>,
}

async fn run_status_line_command(
    command: Vec<String>,
    request: StatusLineRequest,
    timeout_duration: Duration,
) -> StatusLineUpdate {
    let git_branch = current_branch_name(&request.cwd).await;
    let input = StatusLineInput {
        model: request.model,
        model_provider: request.model_provider,
        cwd: request.cwd.to_string_lossy().to_string(),
        git_branch,
        task_running: request.task_running,
        review_mode: request.review_mode,
        context_window_percent: request.context_window_percent,
        context_window_used_tokens: request.context_window_used_tokens,
        token_usage: request.token_usage,
    };
    let json = match serde_json::to_vec(&input) {
        Ok(json) => json,
        Err(err) => {
            warn!("status line input serialization failed: {err}");
            return StatusLineUpdate::Failed;
        }
    };

    let Some((program, args)) = command.split_first() else {
        return StatusLineUpdate::Failed;
    };
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            warn!("status line command failed to spawn: {err}");
            return StatusLineUpdate::Failed;
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        if let Err(err) = stdin.write_all(&json).await {
            warn!("status line command stdin write failed: {err}");
            return StatusLineUpdate::Failed;
        }
    }

    let output = match timeout(timeout_duration, child.wait_with_output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => {
            warn!("status line command failed: {err}");
            return StatusLineUpdate::Failed;
        }
        Err(_) => {
            warn!("status line command timed out after {}ms", timeout_duration.as_millis());
            return StatusLineUpdate::Failed;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            "status line command exited with {}: {}",
            output.status,
            stderr.trim()
        );
        return StatusLineUpdate::Failed;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next().map(|s| s.to_string()).filter(|s| !s.is_empty());
    StatusLineUpdate::Updated(line)
}
