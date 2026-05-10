use super::*;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::process::Stdio;

const TRAIL_SCHEMA_VERSION: u64 = 1;
const TRAIL_CONFIG: &str = "trail.jsonl";
const DAEMON_HEARTBEAT_STALE_MS: u128 = 10_000;
const DAEMON_POLL_MS: u64 = 2_000;

#[derive(Clone, Copy)]
pub(super) enum TrailImportance {
    Low,
    Normal,
    High,
}

impl TrailImportance {
    fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
        }
    }
}

#[derive(Clone)]
struct TrailEvent {
    timestamp_ms: u128,
    kind: String,
    summary: String,
    branch: Option<String>,
    head: Option<String>,
    signals: Vec<String>,
    importance: String,
}

#[derive(Clone)]
struct DaemonState {
    pid: u32,
    cwd: String,
    workspace_id: String,
    started_at_ms: u128,
    last_heartbeat_ms: u128,
    trail_path: String,
    stop_requested: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct GitSnapshot {
    branch: Option<String>,
    head: Option<String>,
}

#[allow(dead_code)]
impl App {
    pub(super) fn trail_path() -> PathBuf {
        if let Ok(path) = std::env::var("ALEPH_TRAIL_PATH") {
            return PathBuf::from(path);
        }

        #[cfg(test)]
        {
            if let Ok(dir) = std::env::var("ALEPH_CONFIG_DIR") {
                return PathBuf::from(dir).join(TRAIL_CONFIG);
            }

            return std::env::temp_dir().join(format!(
                "aleph-test-trail-disabled-{}.jsonl",
                std::process::id()
            ));
        }

        #[cfg(not(test))]
        Self::aleph_config_dir().join(TRAIL_CONFIG)
    }

    pub(super) fn daemon_state_path() -> PathBuf {
        if let Ok(path) = std::env::var("ALEPH_DAEMON_STATE_PATH") {
            return PathBuf::from(path);
        }

        let workspace_id = Self::current_workspace_id();

        #[cfg(test)]
        {
            if let Ok(dir) = std::env::var("ALEPH_CONFIG_DIR") {
                return PathBuf::from(dir).join(format!("daemon-{}.json", workspace_id));
            }

            return std::env::temp_dir().join(format!(
                "aleph-test-daemon-disabled-{}-{}.json",
                workspace_id,
                std::process::id()
            ));
        }

        #[cfg(not(test))]
        Self::aleph_config_dir().join(format!("daemon-{}.json", workspace_id))
    }

    pub(super) fn append_trail_event(
        &self,
        kind: &str,
        summary: impl Into<String>,
        targets: Vec<String>,
        importance: TrailImportance,
    ) -> Result<(), String> {
        Self::append_trail_event_for_current_workspace(
            kind,
            "aleph",
            summary.into(),
            targets,
            importance,
        )
    }

    fn append_trail_event_for_current_workspace(
        kind: &str,
        source: &str,
        summary: String,
        targets: Vec<String>,
        importance: TrailImportance,
    ) -> Result<(), String> {
        let cwd = Self::current_workspace_label();
        let workspace_id = Self::workspace_id_for_label(&cwd);
        let repo = Self::capture_repo_context();
        let signals = Self::extract_signals(&summary);
        let timestamp_ms = Self::now_millis();
        let id = Self::trail_event_id(timestamp_ms, kind, &summary);
        let payload = serde_json::json!({
            "schema_version": TRAIL_SCHEMA_VERSION,
            "id": id,
            "timestamp_ms": timestamp_ms as u64,
            "workspace_id": workspace_id,
            "kind": kind,
            "source": source,
            "summary": summary,
            "cwd": cwd,
            "branch": repo.as_ref().and_then(|repo| repo.branch.as_deref()),
            "head": repo.as_ref().and_then(|repo| repo.head.as_deref()),
            "targets": targets,
            "signals": signals,
            "importance": importance.as_str(),
        });

        let path = Self::trail_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create trail directory '{}': {}",
                    parent.display(),
                    error
                )
            })?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| format!("failed to open trail '{}': {}", path.display(), error))?;
        writeln!(file, "{}", payload)
            .map_err(|error| format!("failed to append trail '{}': {}", path.display(), error))
    }

    pub(super) fn trail_lines(&self, query: Option<&str>) -> Vec<String> {
        let mut events = Self::load_trail_events(200);
        if let Some(query) = query.map(str::trim).filter(|query| !query.is_empty()) {
            let lower = query.to_lowercase();
            events.retain(|event| {
                event.summary.to_lowercase().contains(&lower)
                    || event.kind.to_lowercase().contains(&lower)
                    || event.signals.iter().any(|signal| signal.contains(&lower))
            });
        }

        if events.is_empty() {
            return vec![
                String::from("No trail events yet."),
                String::from("Use /daemon start to watch git context, or keep using Aleph."),
            ];
        }

        let mut lines = vec![String::from("Aleph Trail"), String::new()];
        let recurring = Self::recurring_signal_lines(&events, 6);
        if !recurring.is_empty() {
            lines.push(String::from("Recurring signals:"));
            lines.extend(recurring);
            lines.push(String::new());
        }

        lines.push(String::from("Recent turns:"));
        lines.extend(events.iter().rev().take(12).map(|event| {
            let repo = event
                .branch
                .as_deref()
                .or(event.head.as_deref())
                .map(|value| format!(" [{}]", value))
                .unwrap_or_default();
            format!(
                "{}  {}  {}{}",
                event.timestamp_ms, event.kind, event.summary, repo
            )
        }));
        lines
    }

    pub(super) fn run_trail_cli_command(&mut self, args: &[String]) -> Result<Vec<String>, String> {
        let action = args.first().map(|value| value.as_str()).unwrap_or("show");
        match action {
            "show" | "list" => Ok(self.trail_lines(None)),
            "search" => Ok(self.trail_lines(Some(&args.get(1..).unwrap_or(&[]).join(" ")))),
            _ => Ok(self.trail_lines(Some(&args.join(" ")))),
        }
    }

    pub(super) fn run_daemon_cli_command(
        &mut self,
        args: &[String],
    ) -> Result<Vec<String>, String> {
        let action = args.first().map(|value| value.as_str()).unwrap_or("status");
        match action {
            "start" => self.start_trail_daemon(),
            "run" => self.run_trail_daemon_loop(),
            "status" => Ok(self.daemon_status_lines()),
            "stop" => self.stop_trail_daemon(),
            _ => Err(format!(
                "Unknown daemon action '{}'. Try start, run, status, or stop.",
                action
            )),
        }
    }

    pub(super) fn start_trail_daemon(&self) -> Result<Vec<String>, String> {
        let status = self.current_daemon_status();
        if status.running {
            return Ok(vec![
                String::from("Aleph Trail daemon is already running."),
                format!("PID: {}", status.pid.unwrap_or_default()),
                format!("Trail: {}", Self::trail_path().display()),
            ]);
        }

        let exe = std::env::current_exe()
            .map_err(|error| format!("failed to locate current Aleph executable: {}", error))?;
        let child = Command::new(exe)
            .arg("daemon")
            .arg("run")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("failed to start Aleph Trail daemon: {}", error))?;

        let state = Self::new_daemon_state(child.id(), false);
        Self::save_daemon_state(&state)?;
        self.append_trail_event(
            "daemon",
            "Started Aleph Trail daemon.",
            Vec::new(),
            TrailImportance::Normal,
        )?;

        Ok(vec![
            String::from("Started Aleph Trail daemon."),
            format!("PID: {}", child.id()),
            format!("Trail: {}", Self::trail_path().display()),
        ])
    }

    pub(super) fn stop_trail_daemon(&self) -> Result<Vec<String>, String> {
        let mut state = match Self::load_daemon_state() {
            Some(state) => state,
            None => {
                return Ok(vec![String::from(
                    "No Aleph Trail daemon state exists for this workspace.",
                )])
            }
        };
        state.stop_requested = true;
        Self::save_daemon_state(&state)?;
        self.append_trail_event(
            "daemon",
            "Requested Aleph Trail daemon stop.",
            Vec::new(),
            TrailImportance::Normal,
        )?;
        Ok(vec![
            String::from("Requested Aleph Trail daemon stop."),
            String::from("The daemon exits after its next heartbeat."),
        ])
    }

    pub(super) fn daemon_status_lines(&self) -> Vec<String> {
        let status = self.current_daemon_status();
        let mut lines = vec![
            format!(
                "Daemon: {}",
                if status.running {
                    "running"
                } else {
                    "not running"
                }
            ),
            format!("Workspace: {}", Self::current_workspace_label()),
            format!("Workspace ID: {}", Self::current_workspace_id()),
            format!("Trail: {}", Self::trail_path().display()),
        ];

        if let Some(state) = status.state {
            lines.push(format!("PID: {}", state.pid));
            lines.push(format!("Heartbeat: {}", state.last_heartbeat_ms));
            lines.push(format!("Stop requested: {}", state.stop_requested));
            if let Some(reason) = status.reason {
                lines.push(format!("Status detail: {}", reason));
            }
        } else if let Some(reason) = status.reason {
            lines.push(format!("Status detail: {}", reason));
        }
        lines
    }

    fn run_trail_daemon_loop(&self) -> Result<Vec<String>, String> {
        let mut state = Self::new_daemon_state(std::process::id(), false);
        Self::save_daemon_state(&state)?;
        Self::append_trail_event_for_current_workspace(
            "daemon",
            "daemon",
            String::from("Aleph Trail daemon is watching this workspace."),
            Vec::new(),
            TrailImportance::Normal,
        )?;

        let mut previous_git = Self::current_git_snapshot();
        loop {
            thread::sleep(Duration::from_millis(DAEMON_POLL_MS));

            if Self::load_daemon_state()
                .map(|state| state.stop_requested)
                .unwrap_or(false)
            {
                Self::append_trail_event_for_current_workspace(
                    "daemon",
                    "daemon",
                    String::from("Aleph Trail daemon stopped."),
                    Vec::new(),
                    TrailImportance::Normal,
                )?;
                let _ = fs::remove_file(Self::daemon_state_path());
                return Ok(vec![String::from("Aleph Trail daemon stopped.")]);
            }

            let current_git = Self::current_git_snapshot();
            if current_git.branch != previous_git.branch {
                let summary = format!(
                    "Git branch changed from {} to {}.",
                    previous_git.branch.as_deref().unwrap_or("unknown"),
                    current_git.branch.as_deref().unwrap_or("unknown")
                );
                Self::append_trail_event_for_current_workspace(
                    "git_branch",
                    "daemon",
                    summary,
                    Vec::new(),
                    TrailImportance::Normal,
                )?;
            }
            if current_git.head != previous_git.head {
                let summary = format!(
                    "Git HEAD changed from {} to {}.",
                    previous_git.head.as_deref().unwrap_or("unknown"),
                    current_git.head.as_deref().unwrap_or("unknown")
                );
                Self::append_trail_event_for_current_workspace(
                    "git_commit",
                    "daemon",
                    summary,
                    Vec::new(),
                    TrailImportance::Normal,
                )?;
            }
            previous_git = current_git;

            state.last_heartbeat_ms = Self::now_millis();
            Self::save_daemon_state(&state)?;
        }
    }

    fn load_trail_events(limit: usize) -> Vec<TrailEvent> {
        let path = Self::trail_path();
        let Ok(file) = fs::File::open(path) else {
            return Vec::new();
        };
        let reader = BufReader::new(file);
        let mut events = reader
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| Self::trail_event_from_line(&line))
            .collect::<Vec<_>>();
        if events.len() > limit {
            events.drain(0..events.len() - limit);
        }
        events
    }

    fn trail_event_from_line(line: &str) -> Option<TrailEvent> {
        let value: serde_json::Value = serde_json::from_str(line).ok()?;
        if value.get("schema_version")?.as_u64()? != TRAIL_SCHEMA_VERSION {
            return None;
        }
        Some(TrailEvent {
            timestamp_ms: value.get("timestamp_ms")?.as_u64()? as u128,
            kind: value.get("kind")?.as_str()?.to_string(),
            summary: value.get("summary")?.as_str()?.to_string(),
            branch: value
                .get("branch")
                .and_then(|branch| branch.as_str())
                .map(str::to_string),
            head: value
                .get("head")
                .and_then(|head| head.as_str())
                .map(str::to_string),
            signals: value
                .get("signals")
                .and_then(|signals| signals.as_array())
                .map(|signals| {
                    signals
                        .iter()
                        .filter_map(|signal| signal.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            importance: value
                .get("importance")
                .and_then(|importance| importance.as_str())
                .unwrap_or("normal")
                .to_string(),
        })
    }

    fn recurring_signal_lines(events: &[TrailEvent], limit: usize) -> Vec<String> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for event in events.iter().rev().take(80) {
            for signal in &event.signals {
                *counts.entry(signal.clone()).or_insert(0) += match event.importance.as_str() {
                    "high" => 3,
                    "normal" => 2,
                    _ => 1,
                };
            }
        }
        let mut ranked = counts
            .into_iter()
            .filter(|(_, count)| *count >= 2)
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        ranked
            .into_iter()
            .take(limit)
            .map(|(signal, _)| format!("- {}", signal))
            .collect()
    }

    fn extract_signals(summary: &str) -> Vec<String> {
        let mut signals = Vec::new();
        for token in Self::signal_tokens(summary) {
            if token.len() < 3 || Self::is_signal_stopword(&token) {
                continue;
            }
            if !signals.contains(&token) {
                signals.push(token);
            }
            if signals.len() >= 8 {
                break;
            }
        }
        signals
    }

    fn signal_tokens(text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        for character in text.chars().flat_map(char::to_lowercase) {
            if character.is_ascii_alphanumeric() || character == '-' {
                current.push(character);
            } else if !current.is_empty() {
                tokens.push(current.trim_matches('-').to_string());
                current.clear();
            }
        }
        if !current.is_empty() {
            tokens.push(current.trim_matches('-').to_string());
        }
        tokens
            .into_iter()
            .filter(|token| !token.is_empty())
            .collect()
    }

    fn is_signal_stopword(token: &str) -> bool {
        matches!(
            token,
            "and"
                | "the"
                | "for"
                | "from"
                | "with"
                | "this"
                | "that"
                | "into"
                | "note"
                | "notes"
                | "path"
                | "saved"
                | "opened"
                | "trail"
                | "daemon"
                | "changed"
                | "requested"
                | "unknown"
                | "workspace"
        )
    }

    fn current_git_snapshot() -> GitSnapshot {
        GitSnapshot {
            branch: Self::git_output(&["branch", "--show-current"]),
            head: Self::git_output(&["rev-parse", "--short", "HEAD"]),
        }
    }

    fn new_daemon_state(pid: u32, stop_requested: bool) -> DaemonState {
        let now = Self::now_millis();
        let cwd = Self::current_workspace_label();
        DaemonState {
            pid,
            workspace_id: Self::workspace_id_for_label(&cwd),
            cwd,
            started_at_ms: now,
            last_heartbeat_ms: now,
            trail_path: Self::trail_path().display().to_string(),
            stop_requested,
        }
    }

    fn current_daemon_status(&self) -> DaemonStatus {
        let Some(state) = Self::load_daemon_state() else {
            return DaemonStatus {
                running: false,
                pid: None,
                state: None,
                reason: Some(String::from("state file not found")),
            };
        };

        let cwd_matches = state.cwd == Self::current_workspace_label();
        let heartbeat_recent =
            Self::now_millis().saturating_sub(state.last_heartbeat_ms) <= DAEMON_HEARTBEAT_STALE_MS;
        let pid_alive = Self::pid_is_alive(state.pid);
        let running = cwd_matches && heartbeat_recent && pid_alive && !state.stop_requested;
        let reason = if !cwd_matches {
            Some(String::from("state belongs to another cwd"))
        } else if state.stop_requested {
            Some(String::from("stop has been requested"))
        } else if !pid_alive {
            Some(String::from("pid is not alive"))
        } else if !heartbeat_recent {
            Some(String::from("heartbeat is stale"))
        } else {
            None
        };

        DaemonStatus {
            running,
            pid: Some(state.pid),
            state: Some(state),
            reason,
        }
    }

    fn save_daemon_state(state: &DaemonState) -> Result<(), String> {
        let path = Self::daemon_state_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create daemon state directory '{}': {}",
                    parent.display(),
                    error
                )
            })?;
        }
        let payload = serde_json::json!({
            "version": 1,
            "pid": state.pid,
            "cwd": state.cwd,
            "workspace_id": state.workspace_id,
            "started_at_ms": state.started_at_ms as u64,
            "last_heartbeat_ms": state.last_heartbeat_ms as u64,
            "trail_path": state.trail_path,
            "stop_requested": state.stop_requested,
        });
        fs::write(
            &path,
            serde_json::to_string_pretty(&payload)
                .map_err(|error| format!("failed to encode daemon state: {}", error))?,
        )
        .map_err(|error| {
            format!(
                "failed to write daemon state '{}': {}",
                path.display(),
                error
            )
        })
    }

    fn load_daemon_state() -> Option<DaemonState> {
        let path = Self::daemon_state_path();
        let body = fs::read_to_string(path).ok()?;
        let value: serde_json::Value = serde_json::from_str(&body).ok()?;
        Some(DaemonState {
            pid: value.get("pid")?.as_u64()? as u32,
            cwd: value.get("cwd")?.as_str()?.to_string(),
            workspace_id: value.get("workspace_id")?.as_str()?.to_string(),
            started_at_ms: value.get("started_at_ms")?.as_u64()? as u128,
            last_heartbeat_ms: value.get("last_heartbeat_ms")?.as_u64()? as u128,
            trail_path: value.get("trail_path")?.as_str()?.to_string(),
            stop_requested: value
                .get("stop_requested")
                .and_then(|stop| stop.as_bool())
                .unwrap_or(false),
        })
    }

    fn pid_is_alive(pid: u32) -> bool {
        if pid == 0 {
            return false;
        }

        #[cfg(unix)]
        {
            PathBuf::from(format!("/proc/{}", pid)).exists()
        }

        #[cfg(windows)]
        {
            Command::new("cmd")
                .args(["/C", "tasklist", "/FI", &format!("PID eq {}", pid), "/NH"])
                .output()
                .ok()
                .map(|output| {
                    output.status.success()
                        && String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
                })
                .unwrap_or(false)
        }

        #[cfg(not(any(unix, windows)))]
        {
            true
        }
    }

    fn current_workspace_label() -> String {
        std::env::current_dir()
            .ok()
            .and_then(|path| path.canonicalize().ok().or(Some(path)))
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| String::from("unknown"))
    }

    fn current_workspace_id() -> String {
        Self::workspace_id_for_label(&Self::current_workspace_label())
    }

    fn workspace_id_for_label(label: &str) -> String {
        let digest = Sha256::digest(label.as_bytes());
        URL_SAFE_NO_PAD.encode(digest)[..16].to_string()
    }

    fn trail_event_id(timestamp_ms: u128, kind: &str, summary: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(kind.as_bytes());
        hasher.update(summary.as_bytes());
        hasher.update(timestamp_ms.to_string().as_bytes());
        let digest = hasher.finalize();
        format!("trail_{}", &URL_SAFE_NO_PAD.encode(digest)[..16])
    }
}

struct DaemonStatus {
    running: bool,
    pid: Option<u32>,
    state: Option<DaemonState>,
    reason: Option<String>,
}
