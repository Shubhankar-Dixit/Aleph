use super::*;

#[allow(dead_code)]
impl App {
    pub(super) fn start_openrouter_browser_login(&mut self) -> bool {
        if self.openrouter_login_rx.is_some() {
            self.last_action = String::from(
                "OpenRouter provider setup is already running. Use /logout to cancel it first.",
            );
            return false;
        }

        let (sender, receiver) = mpsc::channel();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.openrouter_login_rx = Some(receiver);
        self.openrouter_login_cancel = Some(cancel_flag.clone());
        self.set_result_panel(
            "OpenRouter provider setup",
            vec![
                String::from("A browser window will open for OpenRouter authorization."),
                String::from("After you authorize Aleph, the API key will be stored locally."),
                String::from("If the browser does not open, copy the auth URL from the terminal."),
            ],
        );
        self.last_action = String::from("Starting OpenRouter provider setup.");

        thread::spawn(move || {
            let result = Self::run_openrouter_browser_login_flow(cancel_flag);
            let _ = sender.send(result);
        });

        true
    }

    pub(super) fn run_openrouter_browser_login_flow(
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<String, String> {
        let (code_verifier, code_challenge) = Self::build_pkce_pair();
        let callback_nonce = Self::build_login_nonce();
        let callback_path = format!("{}/{}", OPENROUTER_AUTH_CALLBACK, callback_nonce);
        let callback_url = format!("http://localhost:{}{}", OPENROUTER_AUTH_PORT, callback_path);
        let auth_url = format!(
            "https://openrouter.ai/auth?callback_url={}&code_challenge={}&code_challenge_method=S256",
            urlencoding::encode(&callback_url),
            urlencoding::encode(&code_challenge),
        );

        let listener = TcpListener::bind(("127.0.0.1", OPENROUTER_AUTH_PORT)).map_err(|error| {
            format!(
                "failed to bind local OpenRouter callback listener: {}",
                error
            )
        })?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("failed to configure the callback listener: {}", error))?;

        Self::open_browser(&auth_url)?;

        let deadline = Instant::now() + Duration::from_secs(600);
        let (mut stream, _) = loop {
            if cancel_flag.load(Ordering::Relaxed) {
                return Err(String::from("OpenRouter authorization was canceled."));
            }

            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(String::from(
                            "OpenRouter authorization timed out waiting for the callback.",
                        ));
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    return Err(format!("failed to accept OpenRouter callback: {}", error));
                }
            }
        };

        let code = Self::read_openrouter_callback_code(&mut stream, &callback_path)?;

        Self::write_openrouter_callback_response(
            &mut stream,
            "OpenRouter authorization completed. You can return to Aleph now.",
        )?;

        if cancel_flag.load(Ordering::Relaxed) {
            return Err(String::from("OpenRouter authorization was canceled."));
        }

        Self::exchange_openrouter_code_for_key(&code, &code_verifier)
    }

    pub(super) fn start_strix_browser_login(&mut self) -> bool {
        if self.strix_login_rx.is_some() {
            self.last_action =
                String::from("Strix login is already running. Use /logout to cancel it first.");
            return false;
        }

        let (sender, receiver) = mpsc::channel();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.strix_login_rx = Some(receiver);
        self.strix_login_cancel = Some(cancel_flag.clone());
        self.set_result_panel(
            "Strix browser login",
            vec![
                String::from("A browser window will open for Strix sign-in."),
                String::from(
                    "After you authenticate, Aleph receives a native app token via localhost.",
                ),
                format!("Server: {}", Self::strix_auth_base_url()),
            ],
        );
        self.add_strix_log("Starting browser login");
        self.last_action = String::from("Starting Strix browser login.");

        thread::spawn(move || {
            let result = Self::run_strix_browser_login_flow(cancel_flag);
            let _ = sender.send(result);
        });

        true
    }

    pub(super) fn run_strix_browser_login_flow(
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<String, String> {
        let (code_verifier, code_challenge) = Self::build_pkce_pair();
        let state = Self::build_login_nonce();
        let callback_path = format!("{}/{}", STRIX_AUTH_CALLBACK, state);
        let redirect_uri = format!("http://127.0.0.1:{}{}", STRIX_AUTH_PORT, callback_path);
        let auth_base_url = Self::strix_auth_base_url();
        let auth_url = format!(
            "{}/api/auth/native/start?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
            auth_base_url,
            urlencoding::encode(STRIX_CLIENT_ID),
            urlencoding::encode(&redirect_uri),
            urlencoding::encode("native:session"),
            urlencoding::encode(&state),
            urlencoding::encode(&code_challenge),
        );

        let listener = TcpListener::bind(("127.0.0.1", STRIX_AUTH_PORT))
            .map_err(|error| format!("failed to bind local Strix callback listener: {}", error))?;
        listener.set_nonblocking(true).map_err(|error| {
            format!("failed to configure the Strix callback listener: {}", error)
        })?;

        Self::open_browser(&auth_url)?;

        let deadline = Instant::now() + Duration::from_secs(600);
        let (mut stream, _) = loop {
            if cancel_flag.load(Ordering::Relaxed) {
                return Err(String::from("Strix browser login was canceled."));
            }

            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(String::from(
                            "Strix browser login timed out waiting for the callback.",
                        ));
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    return Err(format!("failed to accept Strix callback: {}", error));
                }
            }
        };

        let request_path = Self::read_oauth_callback_path(&mut stream, &callback_path, "Strix")?;
        if let Some(error) = Self::query_parameter(&request_path, "error") {
            return Err(format!("Strix login returned an error: {}", error));
        }
        let returned_state = Self::query_parameter(&request_path, "state")
            .ok_or_else(|| String::from("Strix callback did not include state"))?;
        if returned_state != state {
            return Err(String::from(
                "Strix callback state did not match the login request.",
            ));
        }
        let code = Self::query_parameter(&request_path, "code")
            .ok_or_else(|| String::from("Strix callback did not include an authorization code"))?;

        Self::write_oauth_callback_response(
            &mut stream,
            "Strix login complete",
            "Strix login completed. You can return to Aleph now.",
            "Strix",
        )?;

        if cancel_flag.load(Ordering::Relaxed) {
            return Err(String::from("Strix browser login was canceled."));
        }

        Self::exchange_strix_code_for_token(&auth_base_url, &code, &code_verifier, &redirect_uri)
    }

    pub(super) fn build_pkce_pair() -> (String, String) {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        let verifier = URL_SAFE_NO_PAD.encode(bytes);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        (verifier, challenge)
    }

    pub(super) fn build_login_nonce() -> String {
        let mut bytes = [0u8; 12];
        OsRng.fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    pub(super) fn open_browser(url: &str) -> Result<(), String> {
        if cfg!(target_os = "windows") {
            Command::new("cmd")
                .arg("/C")
                .arg("start")
                .arg("")
                .arg(url.replace("&", "^&"))
                .spawn()
                .map_err(|error| format!("failed to open the browser: {}", error))?;
            return Ok(());
        }

        if cfg!(target_os = "macos") {
            Command::new("open")
                .arg(url)
                .spawn()
                .map_err(|error| format!("failed to open the browser: {}", error))?;
            return Ok(());
        }

        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|error| format!("failed to open the browser: {}", error))?;
        Ok(())
    }

    pub(super) fn read_openrouter_callback_code(
        stream: &mut std::net::TcpStream,
        expected_path: &str,
    ) -> Result<String, String> {
        Self::read_oauth_callback_parameter(stream, expected_path, "code", "OpenRouter")
    }

    pub(super) fn read_oauth_callback_parameter(
        stream: &mut std::net::TcpStream,
        expected_path: &str,
        parameter: &str,
        provider: &str,
    ) -> Result<String, String> {
        let request_path = Self::read_oauth_callback_path(stream, expected_path, provider)?;
        if let Some(error) = Self::query_parameter(&request_path, "error") {
            return Err(format!("{} login returned an error: {}", provider, error));
        }

        Self::query_parameter(&request_path, parameter)
            .ok_or_else(|| format!("{} callback did not include {}", provider, parameter))
    }

    pub(super) fn read_oauth_callback_path(
        stream: &mut std::net::TcpStream,
        expected_path: &str,
        provider: &str,
    ) -> Result<String, String> {
        let request_path = {
            let mut reader = BufReader::new(stream);
            let mut request_line = String::new();
            reader.read_line(&mut request_line).map_err(|error| {
                format!("failed to read {} callback request: {}", provider, error)
            })?;

            let mut header = String::new();
            loop {
                header.clear();
                let bytes_read = reader.read_line(&mut header).map_err(|error| {
                    format!("failed to read {} callback headers: {}", provider, error)
                })?;
                if bytes_read == 0 || header == "\r\n" {
                    break;
                }
            }

            request_line
                .split_whitespace()
                .nth(1)
                .ok_or_else(|| format!("{} callback request did not include a path", provider))?
                .to_string()
        };

        let request_path_only = request_path.split('?').next().unwrap_or(&request_path);
        if request_path_only != expected_path {
            return Err(format!(
                "{} callback arrived on an unexpected path.",
                provider
            ));
        }

        Ok(request_path)
    }

    pub(super) fn write_openrouter_callback_response(
        stream: &mut std::net::TcpStream,
        message: &str,
    ) -> Result<(), String> {
        Self::write_oauth_callback_response(
            stream,
            "OpenRouter authorization complete",
            message,
            "OpenRouter",
        )
    }

    pub(super) fn write_oauth_callback_response(
        stream: &mut std::net::TcpStream,
        title: &str,
        message: &str,
        provider: &str,
    ) -> Result<(), String> {
        let body = format!(
            "<html><body style=\"font-family: sans-serif; padding: 2rem;\"><h1>{}</h1><p>{}</p></body></html>",
            title,
            message
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .map_err(|error| format!("failed to write {} callback response: {}", provider, error))
    }

    pub(super) fn exchange_openrouter_code_for_key(
        code: &str,
        code_verifier: &str,
    ) -> Result<String, String> {
        let payload = serde_json::json!({
            "code": code,
            "code_verifier": code_verifier,
            "code_challenge_method": "S256",
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| format!("failed to build HTTP client: {}", error))?;

        let response = client
            .post("https://openrouter.ai/api/v1/auth/keys")
            .json(&payload)
            .send()
            .map_err(|error| {
                format!(
                    "failed to exchange the OpenRouter authorization code: {}",
                    error
                )
            })?;

        let status = response.status();
        let body = response
            .text()
            .map_err(|error| format!("failed to read OpenRouter auth response: {}", error))?;

        if !status.is_success() {
            return Err(format!("{}: {}", status, body));
        }

        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|error| format!("failed to parse OpenRouter auth response: {}", error))?;

        value
            .get("key")
            .and_then(|key| key.as_str())
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
            .ok_or_else(|| String::from("OpenRouter auth response did not include an API key"))
    }

    pub(super) fn exchange_strix_code_for_token(
        auth_base_url: &str,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
    ) -> Result<String, String> {
        let payload = serde_json::json!({
            "grant_type": "authorization_code",
            "code": code,
            "code_verifier": code_verifier,
            "client_id": STRIX_CLIENT_ID,
            "redirect_uri": redirect_uri,
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| format!("failed to build HTTP client: {}", error))?;

        let response = client
            .post(format!("{}/api/auth/native/token", auth_base_url))
            .json(&payload)
            .send()
            .map_err(|error| {
                format!("failed to exchange the Strix authorization code: {}", error)
            })?;

        let status = response.status();
        let body = response
            .text()
            .map_err(|error| format!("failed to read Strix auth response: {}", error))?;

        if !status.is_success() {
            return Err(format!("{}: {}", status, body));
        }

        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|error| format!("failed to parse Strix auth response: {}", error))?;

        value
            .get("access_token")
            .and_then(|token| token.as_str())
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty())
            .ok_or_else(|| String::from("Strix auth response did not include an access token"))
    }

    pub(super) fn query_parameter(path: &str, name: &str) -> Option<String> {
        let query = path.split_once('?')?.1;

        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            if key == name {
                return urlencoding::decode(value)
                    .ok()
                    .map(|decoded| decoded.into_owned());
            }
        }

        None
    }

    pub(super) fn parse_chat_markdown_spans_owned(text: &str) -> Vec<Span<'static>> {
        let mut spans = Vec::new();
        let mut remaining = text;

        while !remaining.is_empty() {
            if let Some(pos) = remaining.find("**") {
                if pos > 0 {
                    spans.push(Span::raw(remaining[..pos].to_string()));
                }
                remaining = &remaining[pos + 2..];
                if let Some(end_pos) = remaining.find("**") {
                    spans.push(Span::styled(
                        remaining[..end_pos].to_string(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                    remaining = &remaining[end_pos + 2..];
                } else {
                    spans.push(Span::raw("**"));
                    spans.push(Span::raw(remaining.to_string()));
                    break;
                }
            } else if let Some(pos) = remaining.find('*') {
                if pos > 0 {
                    spans.push(Span::raw(remaining[..pos].to_string()));
                }
                remaining = &remaining[pos + 1..];
                if let Some(end_pos) = remaining.find('*') {
                    spans.push(Span::styled(
                        remaining[..end_pos].to_string(),
                        Style::default().add_modifier(Modifier::ITALIC),
                    ));
                    remaining = &remaining[end_pos + 1..];
                } else {
                    spans.push(Span::raw("*"));
                    spans.push(Span::raw(remaining.to_string()));
                    break;
                }
            } else if let Some(pos) = remaining.find('`') {
                if pos > 0 {
                    spans.push(Span::raw(remaining[..pos].to_string()));
                }
                remaining = &remaining[pos + 1..];
                if let Some(end_pos) = remaining.find('`') {
                    spans.push(Span::styled(
                        remaining[..end_pos].to_string(),
                        Style::default().fg(CHAT_ACCENT_SOFT),
                    ));
                    remaining = &remaining[end_pos + 1..];
                } else {
                    spans.push(Span::raw("`"));
                    spans.push(Span::raw(remaining.to_string()));
                    break;
                }
            } else {
                spans.push(Span::raw(remaining.to_string()));
                break;
            }
        }

        if spans.is_empty() {
            spans.push(Span::raw(text.to_string()));
        }

        spans
    }

    pub(super) fn render_chat_markdown_line_owned(line: &str) -> Line<'static> {
        let mut spans = Vec::new();
        let mut remaining = line;
        let trimmed = line.trim_start();
        let indent_len = line.len() - trimmed.len();

        if trimmed.starts_with("# ") {
            spans.push(Span::styled(
                line[..indent_len + 2].to_string(),
                Style::default().fg(CHAT_ACCENT_SOFT),
            ));
            spans.push(Span::styled(
                trimmed[2..].to_string(),
                Style::default()
                    .fg(CHAT_TEXT)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
            return Line::from(spans);
        } else if trimmed.starts_with("## ") {
            spans.push(Span::styled(
                line[..indent_len + 3].to_string(),
                Style::default().fg(CHAT_MUTED),
            ));
            spans.push(Span::styled(
                trimmed[3..].to_string(),
                Style::default().fg(CHAT_TEXT).add_modifier(Modifier::BOLD),
            ));
            return Line::from(spans);
        } else if trimmed.starts_with("### ") {
            spans.push(Span::styled(
                line[..indent_len + 4].to_string(),
                Style::default().fg(CHAT_MUTED),
            ));
            spans.push(Span::styled(
                trimmed[4..].to_string(),
                Style::default()
                    .fg(CHAT_TEXT)
                    .add_modifier(Modifier::BOLD | Modifier::ITALIC),
            ));
            return Line::from(spans);
        } else if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let pipe_count = trimmed.chars().filter(|&c| c == '|').count();
            if pipe_count >= 2 {
                let is_separator = trimmed
                    .trim_start_matches('|')
                    .trim_end_matches('|')
                    .split('|')
                    .all(|cell| {
                        cell.trim()
                            .chars()
                            .all(|c| c == '-' || c == ':' || c == ' ')
                    });
                if is_separator {
                    return Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(CHAT_MUTED),
                    ));
                }
                let mut table_spans = Vec::new();
                if indent_len > 0 {
                    table_spans.push(Span::raw(line[..indent_len].to_string()));
                }
                let parts: Vec<&str> = trimmed.split('|').collect();
                for (i, part) in parts.iter().enumerate() {
                    if i == 0 && part.is_empty() {
                        table_spans.push(Span::styled("|", Style::default().fg(CHAT_MUTED)));
                    } else if i == parts.len() - 1 && part.is_empty() {
                        // trailing empty after last pipe
                    } else {
                        table_spans.push(Span::styled(
                            part.to_string(),
                            Style::default().fg(CHAT_TEXT),
                        ));
                        if i < parts.len() - 1 {
                            table_spans.push(Span::styled("|", Style::default().fg(CHAT_MUTED)));
                        }
                    }
                }
                return Line::from(table_spans);
            }
            remaining = trimmed;
        } else if let Some(stripped) = trimmed.strip_prefix("- ") {
            if indent_len > 0 {
                spans.push(Span::raw(line[..indent_len].to_string()));
            }
            spans.push(Span::styled("• ", Style::default().fg(CHAT_ACCENT)));
            remaining = stripped;
        } else if let Some(stripped) = trimmed.strip_prefix("* ") {
            if indent_len > 0 {
                spans.push(Span::raw(line[..indent_len].to_string()));
            }
            spans.push(Span::styled("• ", Style::default().fg(CHAT_ACCENT)));
            remaining = stripped;
        } else if let Some(pos) = trimmed.find(". ") {
            if pos > 0 && trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
                if indent_len > 0 {
                    spans.push(Span::raw(line[..indent_len].to_string()));
                }
                spans.push(Span::styled(
                    trimmed[..=pos + 1].to_string(),
                    Style::default().fg(CHAT_ACCENT),
                ));
                remaining = &trimmed[pos + 2..];
            }
        }

        spans.extend(Self::parse_chat_markdown_spans_owned(remaining));
        Line::from(spans)
    }

    pub(super) fn render_chat_markdown_lines_owned(content: &str) -> Vec<Line<'static>> {
        let mut rendered = Vec::new();
        let lines = content.lines().collect::<Vec<_>>();
        let mut index = 0;

        while index < lines.len() {
            let line = lines[index];
            if line.is_empty() {
                rendered.push(Line::from(""));
                index += 1;
                continue;
            }

            if Self::is_chat_table_line(line) {
                let start = index;
                while index < lines.len() && Self::is_chat_table_line(lines[index]) {
                    index += 1;
                }
                for table_line in Self::format_chat_table_block(&lines[start..index]) {
                    rendered.push(Self::render_chat_markdown_line_owned(&table_line));
                }
                continue;
            }

            rendered.push(Self::render_chat_markdown_line_owned(line));
            index += 1;
        }

        rendered
    }

    pub(super) fn is_chat_table_line(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with('|')
            && trimmed.ends_with('|')
            && trimmed.chars().filter(|&c| c == '|').count() >= 2
    }

    pub(super) fn format_chat_table_block(lines: &[&str]) -> Vec<String> {
        let rows = lines
            .iter()
            .map(|line| {
                line.trim()
                    .trim_matches('|')
                    .split('|')
                    .map(|cell| cell.trim().to_string())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
        if columns == 0 {
            return lines.iter().map(|line| (*line).to_string()).collect();
        }

        let mut widths = vec![0usize; columns];
        for row in &rows {
            if Self::is_chat_table_separator_cells(row) {
                continue;
            }
            for (column, cell) in row.iter().enumerate() {
                widths[column] = widths[column].max(cell.chars().count());
            }
        }
        for width in &mut widths {
            *width = (*width).max(3);
        }

        rows.into_iter()
            .map(|row| {
                if Self::is_chat_table_separator_cells(&row) {
                    let cells = widths
                        .iter()
                        .map(|width| "-".repeat(*width))
                        .collect::<Vec<_>>();
                    return format!("| {} |", cells.join(" | "));
                }

                let cells = (0..columns)
                    .map(|column| {
                        let cell = row.get(column).map(String::as_str).unwrap_or("");
                        let padding = widths[column].saturating_sub(cell.chars().count());
                        format!("{}{}", cell, " ".repeat(padding))
                    })
                    .collect::<Vec<_>>();
                format!("| {} |", cells.join(" | "))
            })
            .collect()
    }

    fn is_chat_table_separator_cells(cells: &[String]) -> bool {
        !cells.is_empty()
            && cells.iter().all(|cell| {
                let trimmed = cell.trim();
                !trimmed.is_empty()
                    && trimmed
                        .chars()
                        .all(|c| c == '-' || c == ':' || c.is_whitespace())
            })
    }

    pub(super) fn start_chat_turn(&mut self, query: String) -> bool {
        let query = query.trim().to_string();
        if query.is_empty() {
            return false;
        }

        if self.chat_stream_rx.is_some() {
            self.last_action = String::from("Aleph is still answering the previous message.");
            return false;
        }

        let provider = self.ai_provider;
        let openrouter_api_key = self.openrouter_api_key.clone();
        let strix_access_token = self.strix_access_token.clone();

        self.push_chat_message("user", query.clone());

        let conversation = match provider {
            AiProvider::OpenRouter => self.openrouter_conversation(&query),
            AiProvider::Strix => Vec::new(),
        };
        let strix_notes = if provider == AiProvider::Strix {
            self.notes.clone()
        } else {
            Vec::new()
        };

        self.push_chat_message("assistant", String::new());

        self.panel_mode = PanelMode::AiChat;
        self.thinking = true;
        self.thinking_status = String::from("Reading workspace context...");
        self.thinking_ticks_remaining = 20;
        self.chat_scroll_offset = 0;
        self.streaming_buffer.clear();
        self.streaming_active = true;
        self.last_action = format!("AI Chat: {}", query);
        self.add_activity(format!("User asked: {}", Self::preview_text(&query, 72)));
        self.add_activity("Reading selected note and recent messages.");
        self.add_activity(format!("Sending request to {}.", self.ai_provider_label()));

        let (sender, receiver) = mpsc::channel();
        self.chat_stream_rx = Some(receiver);

        match provider {
            AiProvider::OpenRouter => {
                let Some(api_key) = openrouter_api_key else {
                    let _ = sender.send(ChatStreamUpdate::Error(String::from(
                        "OpenRouter is not configured as a model provider. Run /login openrouter first.",
                    )));
                    return true;
                };
                thread::spawn(move || {
                    if let Err(error) = Self::send_openrouter_chat_streaming(
                        &api_key,
                        &conversation,
                        sender.clone(),
                    ) {
                        let _ = sender.send(ChatStreamUpdate::Error(error));
                    }
                });
            }
            AiProvider::Strix => {
                let Some(access_token) = strix_access_token else {
                    let _ = sender.send(ChatStreamUpdate::Error(String::from(
                        "Strix is not connected. Run /login strix first.",
                    )));
                    return true;
                };
                let base_url = Self::strix_api_base_url();
                thread::spawn(move || {
                    if let Err(error) = Self::send_strix_chat(
                        &base_url,
                        &access_token,
                        &query,
                        &strix_notes,
                        sender.clone(),
                    ) {
                        let _ = sender.send(ChatStreamUpdate::Error(error));
                    }
                });
            }
        }

        true
    }

    pub(super) fn openrouter_conversation(&self, query: &str) -> Vec<(String, String)> {
        let mut conversation = Vec::new();
        conversation.push((
            String::from("system"),
            String::from("You are Aleph, a concise terminal assistant. Keep answers practical and grounded. Use the provided workspace context when it is relevant, and say when the local notes or memories do not contain enough evidence."),
        ));
        conversation.push((
            String::from("system"),
            self.agent_workspace_context_for_query(query),
        ));

        let mut recent_messages: Vec<_> =
            self.chat_messages.iter().rev().take(12).cloned().collect();
        recent_messages.reverse();

        for message in recent_messages {
            if message.content.trim().is_empty() {
                continue;
            }
            conversation.push((message.role, message.content));
        }

        conversation
    }

    pub(super) fn rebuild_chat_render_cache(&mut self) {
        let mut lines: Vec<Line<'static>> = Vec::new();

        if self.chat_messages.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                if self.is_openrouter_connected() || self.is_strix_connected() {
                    "Welcome to Aleph AI chat. Type a message below to start."
                } else {
                    "Welcome to Aleph AI chat. Run /login to sign in."
                },
                Style::default().fg(CHAT_MUTED),
            )]));
            self.chat_render_cache = lines;
            self.chat_cache_stable_len = self.chat_render_cache.len();
            return;
        }

        let msg_count = self.chat_messages.len();
        for (index, message) in self.chat_messages.iter().enumerate() {
            if index > 0 {
                lines.push(Line::from(""));
            }

            let is_user = message.role == "user";
            let prefix = if is_user { "You" } else { "Aleph" };
            let color = if is_user {
                CHAT_ACCENT_SOFT
            } else {
                CHAT_ACCENT
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{} ", prefix),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("({})", message.timestamp),
                    Style::default().fg(CHAT_MUTED),
                ),
            ]));

            // Mark stable length right after the last message's header
            if index == msg_count - 1 {
                self.chat_cache_stable_len = lines.len();
            }

            if message.content.trim().is_empty() {
                continue;
            }

            lines.extend(Self::render_chat_markdown_lines_owned(&message.content));
        }

        self.chat_render_cache = lines;
    }

    /// Fast incremental rebuild: only re-render the last message's content.
    /// Used during streaming so we don't re-parse every previous message's
    /// markdown on each token from the model.
    pub(super) fn rebuild_chat_render_cache_streaming(&mut self) {
        let old_len = self.chat_render_cache.len();
        self.chat_render_cache.truncate(self.chat_cache_stable_len);

        if let Some(last_msg) = self.chat_messages.last() {
            if !last_msg.content.trim().is_empty() {
                self.chat_render_cache
                    .extend(Self::render_chat_markdown_lines_owned(&last_msg.content));
            }
        }

        if self.chat_scroll_offset > 0 {
            let delta = self.chat_render_cache.len().saturating_sub(old_len);
            self.chat_scroll_offset = self.chat_scroll_offset.saturating_add(delta);
        }
    }

    pub(super) fn send_openrouter_chat_streaming(
        api_key: &str,
        conversation: &[(String, String)],
        sender: Sender<ChatStreamUpdate>,
    ) -> Result<(), String> {
        let messages: Vec<_> = conversation
            .iter()
            .map(|(role, content)| {
                serde_json::json!({
                    "role": role,
                    "content": content,
                })
            })
            .collect();

        let payload = serde_json::json!({
            "model": OPENROUTER_CHAT_MODEL,
            "messages": messages,
            "temperature": 0.7,
            "stream": true,
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(1800))
            .build()
            .map_err(|error| format!("failed to build HTTP client: {}", error))?;

        let response = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .bearer_auth(api_key)
            .header("Accept", "text/event-stream")
            .json(&payload)
            .send()
            .map_err(|error| format!("request failed: {}", error))?;

        let status = response.status();

        if !status.is_success() {
            let body = response
                .text()
                .map_err(|error| format!("failed to read response: {}", error))?;
            return Err(format!("{}: {}", status, body));
        }

        let mut reader = BufReader::with_capacity(256, response);
        let mut line = String::new();
        let mut event_data = String::new();

        loop {
            line.clear();
            let bytes_read = reader
                .read_line(&mut line)
                .map_err(|error| format!("failed to read OpenRouter stream: {}", error))?;

            if bytes_read == 0 {
                break;
            }

            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                if !event_data.is_empty()
                    && Self::handle_openrouter_stream_event(&event_data, &sender)?
                {
                    return Ok(());
                }
                event_data.clear();
                continue;
            }

            if trimmed.starts_with(':') {
                continue;
            }

            if let Some(payload) = trimmed.strip_prefix("data:") {
                if !event_data.is_empty() {
                    event_data.push('\n');
                }
                event_data.push_str(payload.strip_prefix(' ').unwrap_or(payload));
            }
        }

        if !event_data.is_empty() {
            let _ = Self::handle_openrouter_stream_event(&event_data, &sender)?;
        }

        let _ = sender.send(ChatStreamUpdate::Done);
        Ok(())
    }

    pub(super) fn handle_openrouter_stream_event(
        event_data: &str,
        sender: &Sender<ChatStreamUpdate>,
    ) -> Result<bool, String> {
        let trimmed = event_data.trim();
        if trimmed.is_empty() {
            return Ok(false);
        }

        if trimmed == "[DONE]" {
            let _ = sender.send(ChatStreamUpdate::Done);
            return Ok(true);
        }

        let value: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|error| format!("failed to parse OpenRouter stream chunk: {}", error))?;

        if let Some(error) = value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(|message| message.as_str())
        {
            let _ = sender.send(ChatStreamUpdate::Error(error.to_string()));
            return Ok(true);
        }

        if let Some(choice) = value.get("choices").and_then(|choices| choices.get(0)) {
            if let Some(content) = choice
                .get("delta")
                .and_then(|delta| delta.get("content"))
                .and_then(|content| content.as_str())
            {
                if !content.is_empty() {
                    let _ = sender.send(ChatStreamUpdate::Delta(content.to_string()));
                }
            }

            if let Some(finish_reason) = choice
                .get("finish_reason")
                .and_then(|reason| reason.as_str())
            {
                if finish_reason == "error" {
                    let message = value
                        .get("error")
                        .and_then(|error| error.get("message"))
                        .and_then(|message| message.as_str())
                        .unwrap_or("OpenRouter reported a streaming error");
                    let _ = sender.send(ChatStreamUpdate::Error(message.to_string()));
                } else {
                    let _ = sender.send(ChatStreamUpdate::Done);
                }
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub(super) fn send_openrouter_chat_blocking(
        api_key: &str,
        conversation: &[(String, String)],
    ) -> Result<String, String> {
        let messages: Vec<_> = conversation
            .iter()
            .map(|(role, content)| {
                serde_json::json!({
                    "role": role,
                    "content": content,
                })
            })
            .collect();

        let payload = serde_json::json!({
            "model": OPENROUTER_CHAT_MODEL,
            "messages": messages,
            "temperature": 0.1,
            "stream": false,
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(45))
            .build()
            .map_err(|error| format!("failed to build HTTP client: {}", error))?;

        let response = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .bearer_auth(api_key)
            .json(&payload)
            .send()
            .map_err(|error| format!("request failed: {}", error))?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|error| format!("failed to read response: {}", error))?;

        if !status.is_success() {
            return Err(format!("{}: {}", status, body));
        }

        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|error| format!("failed to parse OpenRouter response: {}", error))?;
        value
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .map(|content| content.trim().to_string())
            .filter(|content| !content.is_empty())
            .ok_or_else(|| String::from("OpenRouter returned an empty planner response"))
    }

    pub fn is_openrouter_connected(&self) -> bool {
        self.connected && self.openrouter_api_key.is_some()
    }

    pub fn is_strix_connected(&self) -> bool {
        self.connected && self.strix_access_token.is_some()
    }

    pub(super) fn load_strix_access_token() -> Option<String> {
        if let Ok(entry) = Self::strix_token_entry() {
            if let Ok(password) = entry.get_password() {
                let trimmed = password.trim().to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        }

        if let Ok(token) = fs::read_to_string(Self::strix_token_path()) {
            let trimmed = token.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }

        std::env::var("STRIX_ACCESS_TOKEN")
            .ok()
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty())
    }

    pub(super) fn store_strix_access_token(&self, access_token: &str) -> Result<(), String> {
        if let Ok(entry) = Self::strix_token_entry() {
            if entry.set_password(access_token.trim()).is_ok() {
                let _ = fs::remove_file(Self::strix_token_path());
                return Ok(());
            }
        }

        let token_path = Self::strix_token_path();
        if let Some(parent) = token_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create Strix token directory '{}': {}",
                    parent.display(),
                    error
                )
            })?;
        }
        fs::write(&token_path, access_token.trim()).map_err(|error| {
            format!(
                "failed to save Strix login fallback '{}': {}",
                token_path.display(),
                error
            )
        })
    }

    pub(super) fn clear_strix_access_token(&self) {
        if let Ok(entry) = Self::strix_token_entry() {
            let _ = entry.delete_credential();
        }
        let _ = fs::remove_file(Self::strix_token_path());
    }

    pub(super) fn strix_token_entry() -> Result<Entry, String> {
        Entry::new(STRIX_SERVICE, STRIX_ACCOUNT)
            .map_err(|error| format!("failed to open Strix credential store: {}", error))
    }

    pub(super) fn strix_token_path() -> PathBuf {
        Self::aleph_config_dir().join(STRIX_TOKEN_CONFIG)
    }

    pub(super) fn strix_auth_base_url() -> String {
        std::env::var("STRIX_AUTH_BASE_URL")
            .ok()
            .map(|url| url.trim().trim_end_matches('/').to_string())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| String::from(STRIX_AUTH_BASE_URL))
    }

    pub(super) fn strix_api_base_url() -> String {
        std::env::var("STRIX_API_BASE_URL")
            .ok()
            .map(|url| url.trim().trim_end_matches('/').to_string())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(Self::strix_auth_base_url)
    }

    pub(super) fn strix_access_token(&self) -> Result<&str, String> {
        self.strix_access_token
            .as_deref()
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| String::from("Strix is not connected. Run /login strix first."))
    }

    pub(super) fn strix_json_request(
        &self,
        method: &str,
        path: &str,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let token = self.strix_access_token()?;
        Self::strix_json_request_with(&Self::strix_api_base_url(), token, method, path, payload)
    }

    pub(super) fn strix_json_request_with(
        base_url: &str,
        token: &str,
        method: &str,
        path: &str,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| format!("failed to build HTTP client: {}", error))?;
        let url = format!("{}{}", base_url.trim_end_matches('/'), path);
        let mut request = match method {
            "GET" => client.get(url),
            "POST" => client.post(url),
            "PATCH" => client.patch(url),
            _ => return Err(format!("unsupported Strix HTTP method: {}", method)),
        }
        .bearer_auth(token);

        if let Some(payload) = payload {
            request = request.json(&payload);
        }

        let response = request
            .send()
            .map_err(|error| format!("Strix request failed: {}", error))?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|error| format!("failed to read Strix response: {}", error))?;
        if !status.is_success() {
            return Err(format!("Strix returned {}: {}", status, body));
        }
        serde_json::from_str(&body)
            .map_err(|error| format!("failed to parse Strix response: {}", error))
    }

    pub(super) fn send_strix_chat(
        base_url: &str,
        token: &str,
        query: &str,
        notes: &[Note],
        sender: Sender<ChatStreamUpdate>,
    ) -> Result<(), String> {
        let notes_payload: Vec<_> = notes
            .iter()
            .take(STRIX_NOTES_LIMIT)
            .map(|note| {
                let id = note
                    .remote_id
                    .clone()
                    .unwrap_or_else(|| note.id.to_string());
                serde_json::json!({
                    "id": id,
                    "title": note.title.as_str(),
                    "content": note.content.as_str(),
                })
            })
            .collect();
        let payload = serde_json::json!({
            "question": query,
            "notes": notes_payload,
        });
        let value =
            Self::strix_json_request_with(base_url, token, "POST", "/nest/ask", Some(payload))?;
        let answer = value
            .get("answer")
            .or_else(|| value.get("result").and_then(|result| result.get("answer")))
            .or_else(|| value.get("content"))
            .and_then(|answer| answer.as_str())
            .unwrap_or("Strix returned an empty answer.")
            .to_string();
        let _ = sender.send(ChatStreamUpdate::Delta(answer));
        let _ = sender.send(ChatStreamUpdate::Done);
        Ok(())
    }

    pub(super) fn send_strix_planner_request(
        base_url: &str,
        token: &str,
        conversation: &[(String, String)],
        notes: &[Note],
    ) -> Result<String, String> {
        let query = conversation
            .iter()
            .map(|(role, content)| format!("{}:\n{}", role, content))
            .collect::<Vec<_>>()
            .join("\n\n");
        let notes_payload: Vec<_> = notes
            .iter()
            .take(STRIX_NOTES_LIMIT)
            .map(|note| {
                let id = note
                    .remote_id
                    .clone()
                    .unwrap_or_else(|| note.id.to_string());
                serde_json::json!({
                    "id": id,
                    "title": note.title.as_str(),
                    "content": note.content.as_str(),
                })
            })
            .collect();
        let payload = serde_json::json!({
            "question": query,
            "notes": notes_payload,
        });
        let value =
            Self::strix_json_request_with(base_url, token, "POST", "/nest/ask", Some(payload))?;
        value
            .get("answer")
            .or_else(|| value.get("result").and_then(|result| result.get("answer")))
            .or_else(|| value.get("content"))
            .and_then(|answer| answer.as_str())
            .map(|answer| answer.trim().to_string())
            .filter(|answer| !answer.is_empty())
            .ok_or_else(|| String::from("Strix returned an empty planner response"))
    }

    pub(super) fn sync_strix_notes(&mut self) -> Result<usize, String> {
        let remote_notes = self.load_strix_notes("", STRIX_NOTES_LIMIT)?;
        let count = remote_notes.len();
        self.merge_strix_notes(remote_notes);
        self.selected_note = 0;
        Self::save_cached_strix_notes(&self.notes)?;
        Self::save_local_notes(&self.notes)?;
        self.add_strix_log(format!("Synced {} notes", count));
        Ok(count)
    }

    pub(super) fn merge_strix_notes(&mut self, remote_notes: Vec<Note>) {
        let existing_by_remote_id: HashMap<String, Note> = self
            .notes
            .iter()
            .filter_map(|note| {
                note.remote_id
                    .as_ref()
                    .map(|remote_id| (remote_id.clone(), note.clone()))
            })
            .collect();
        let mut merged = Vec::with_capacity(remote_notes.len() + self.notes.len());
        let mut remote_ids = Vec::new();

        for mut note in remote_notes {
            if let Some(remote_id) = note.remote_id.clone() {
                if let Some(existing) = existing_by_remote_id.get(&remote_id) {
                    note.id = existing.id;
                    if note.obsidian_path.is_none() {
                        note.obsidian_path = existing.obsidian_path.clone();
                    }
                    if note.folder_id.is_none() {
                        note.folder_id = existing.folder_id;
                    }
                }
                remote_ids.push(remote_id);
            }
            merged.push(note);
        }

        for note in &self.notes {
            let is_matched_remote_note = note
                .remote_id
                .as_ref()
                .map(|remote_id| remote_ids.iter().any(|id| id == remote_id))
                .unwrap_or(false);
            if !is_matched_remote_note {
                merged.push(note.clone());
            }
        }

        for (index, note) in merged.iter_mut().enumerate() {
            note.id = index + 1;
        }
        self.notes = merged;
    }
}
