use std::collections::HashMap;
use std::io::stdout;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState,
        Tabs, Wrap,
    },
    Frame, Terminal,
};
use serde_json::{json, Value};
use tui_textarea::TextArea;

const TEAL: Color = Color::Rgb(0, 161, 155);
const DARK_TEAL: Color = Color::Rgb(0, 100, 96);
const FG: Color = Color::White;
const DIM: Color = Color::Rgb(150, 150, 150);
const RED: Color = Color::Red;
const GREEN: Color = Color::Green;
const YELLOW: Color = Color::Yellow;
const BG: Color = Color::Reset;

fn teal() -> Style {
    Style::default().fg(TEAL)
}
fn bold_teal() -> Style {
    Style::default().fg(TEAL).add_modifier(Modifier::BOLD)
}
fn selected() -> Style {
    Style::default()
        .bg(TEAL)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD)
}
fn dim() -> Style {
    Style::default().fg(DIM)
}
fn header_style() -> Style {
    Style::default().fg(TEAL).add_modifier(Modifier::BOLD)
}

#[derive(Clone)]
pub struct ApiConfig {
    pub base_url: String,
    pub token: Option<String>,
}

impl ApiConfig {
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .user_agent(concat!("microcodes-cli/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap()
    }

    fn get(&self, path: &str) -> Result<Value, String> {
        let url = self.url(path);
        let resp = Self::client()
            .get(&url)
            .send()
            .map_err(|e| format!("Connection error: {}", e))?;
        parse_response(resp)
    }

    fn get_q(&self, path: &str, params: &[(&str, &str)]) -> Result<Value, String> {
        let mut url = self.url(path);
        if !params.is_empty() {
            let qs: String = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, urlenc(v)))
                .collect::<Vec<_>>()
                .join("&");
            url = format!("{}?{}", url, qs);
        }
        let resp = Self::client()
            .get(&url)
            .send()
            .map_err(|e| format!("Connection error: {}", e))?;
        parse_response(resp)
    }

    fn auth_get(&self, path: &str) -> Result<Value, String> {
        let token = self.token.as_deref().ok_or("Not authenticated")?;
        let url = self.url(path);
        let resp = Self::client()
            .get(&url)
            .header("X-API-Key", token)
            .send()
            .map_err(|e| format!("Connection error: {}", e))?;
        parse_response(resp)
    }

    fn auth_get_q(&self, path: &str, params: &[(&str, &str)]) -> Result<Value, String> {
        let token = self.token.as_deref().ok_or("Not authenticated")?;
        let mut url = self.url(path);
        if !params.is_empty() {
            let qs: String = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, urlenc(v)))
                .collect::<Vec<_>>()
                .join("&");
            url = format!("{}?{}", url, qs);
        }
        let resp = Self::client()
            .get(&url)
            .header("X-API-Key", token)
            .send()
            .map_err(|e| format!("Connection error: {}", e))?;
        parse_response(resp)
    }

    fn auth_post(&self, path: &str, body: Value) -> Result<Value, String> {
        let token = self.token.as_deref().ok_or("Not authenticated")?;
        let url = self.url(path);
        let resp = Self::client()
            .post(&url)
            .header("X-API-Key", token)
            .json(&body)
            .send()
            .map_err(|e| format!("Connection error: {}", e))?;
        parse_response(resp)
    }

    fn auth_put(&self, path: &str, body: Value) -> Result<Value, String> {
        let token = self.token.as_deref().ok_or("Not authenticated")?;
        let url = self.url(path);
        let resp = Self::client()
            .put(&url)
            .header("X-API-Key", token)
            .json(&body)
            .send()
            .map_err(|e| format!("Connection error: {}", e))?;
        parse_response(resp)
    }

    fn auth_patch(&self, path: &str, body: Value) -> Result<Value, String> {
        let token = self.token.as_deref().ok_or("Not authenticated")?;
        let url = self.url(path);
        let resp = Self::client()
            .patch(&url)
            .header("X-API-Key", token)
            .json(&body)
            .send()
            .map_err(|e| format!("Connection error: {}", e))?;
        parse_response(resp)
    }

    fn auth_delete(&self, path: &str) -> Result<Value, String> {
        let token = self.token.as_deref().ok_or("Not authenticated")?;
        let url = self.url(path);
        let resp = Self::client()
            .delete(&url)
            .header("X-API-Key", token)
            .send()
            .map_err(|e| format!("Connection error: {}", e))?;
        parse_response(resp)
    }
}

fn parse_response(resp: reqwest::blocking::Response) -> Result<Value, String> {
    let status = resp.status();
    let text = resp.text().map_err(|e| format!("Read error: {}", e))?;
    let body: Value = if text.is_empty() {
        json!({"success": true})
    } else {
        serde_json::from_str(&text).unwrap_or(Value::String(text.clone()))
    };
    if status.is_success() {
        return Ok(body);
    }
    let msg = body
        .get("error")
        .or_else(|| body.get("message"))
        .and_then(|v| v.as_str())
        .unwrap_or(text.as_str());
    Err(match status.as_u16() {
        401 => "Not authenticated. Set MICROCODES_API_TOKEN.".to_string(),
        403 => "Forbidden.".to_string(),
        404 => "Not found.".to_string(),
        c => format!("HTTP {}: {}", c, msg),
    })
}

fn urlenc(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => vec![c],
            ' ' => vec!['+'],
            c => format!("%{:02X}", c as u32).chars().collect(),
        })
        .collect()
}

fn sv(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}
fn nv(v: &Value, k: &str) -> String {
    v.get(k)
        .and_then(|x| x.as_i64())
        .map(|n| n.to_string())
        .unwrap_or_default()
}
fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    }
}

fn ext_to_language(ext: &str) -> Option<&'static str> {
    match ext {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "js" | "mjs" | "cjs" => Some("javascript"),
        "ts" | "mts" | "cts" => Some("typescript"),
        "go" => Some("go"),
        "java" => Some("java"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" => Some("c++"),
        "cs" => Some("c#"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        "swift" => Some("swift"),
        "kt" | "kts" => Some("kotlin"),
        "sh" | "bash" => Some("bash"),
        "ps1" => Some("powershell"),
        "html" | "htm" => Some("html"),
        "css" => Some("css"),
        "scss" | "sass" => Some("scss"),
        "json" => Some("json"),
        "yaml" | "yml" => Some("yaml"),
        "toml" => Some("toml"),
        "sql" => Some("sql"),
        "lua" => Some("lua"),
        "r" => Some("r"),
        "ex" | "exs" => Some("elixir"),
        "hs" => Some("haskell"),
        "clj" | "cljs" => Some("clojure"),
        "ml" | "mli" => Some("ocaml"),
        "dart" => Some("dart"),
        "zig" => Some("zig"),
        _ => None,
    }
}

fn collect_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files(&path, out);
            } else {
                out.push(path);
            }
        }
    }
}

fn build_folder_payload(folder: &str) -> Result<Value, String> {
    let folder_path = Path::new(folder);
    if !folder_path.is_dir() {
        return Err(format!("'{}' is not a directory", folder));
    }

    let meta_path = folder_path.join("meta.json");
    let meta_raw = std::fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read meta.json: {}", e))?;
    let mut payload: Value =
        serde_json::from_str(&meta_raw).map_err(|e| format!("Invalid JSON in meta.json: {}", e))?;

    let mut records: Vec<Value> = Vec::new();
    let mut lang_counts: HashMap<&'static str, usize> = HashMap::new();

    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    collect_files(folder_path, &mut paths);
    paths.sort();

    for path in &paths {
        if path.file_name().map(|n| n == "meta.json").unwrap_or(false)
            && path.parent().map(|p| p == folder_path).unwrap_or(false)
        {
            continue;
        }

        let rel = path
            .strip_prefix(folder_path)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");

        let code = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read '{}': {}", rel, e))?;

        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if let Some(lang) = ext_to_language(ext) {
                *lang_counts.entry(lang).or_insert(0) += 1;
            }
        }

        records.push(json!({ "type": "code", "name": rel, "payload": code }));
    }

    if !records.is_empty() {
        payload["records"] = Value::Array(records);
    }

    if let Some(dominant) = lang_counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(l, _)| l)
    {
        payload["language"] = json!({ "name": dominant });
    }

    Ok(payload)
}

fn parse_command_line(line: &str) -> Result<Vec<String>, String> {
    let mut args: Vec<String> = vec![];
    let mut cur = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    for c in line.chars() {
        if escaped {
            cur.push(c);
            escaped = false;
            continue;
        }
        match c {
            '\\' if !in_single => escaped = true,
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            c if c.is_whitespace() && !in_single && !in_double => {
                if !cur.is_empty() {
                    args.push(cur.clone());
                    cur.clear();
                }
            }
            _ => cur.push(c),
        }
    }

    if escaped {
        cur.push('\\');
    }
    if in_single || in_double {
        return Err("Unclosed quote in command.".to_string());
    }
    if !cur.is_empty() {
        args.push(cur);
    }
    Ok(args)
}

fn execute_cli_command(command_line: &str, api: &ApiConfig) -> Result<CommandRunResult, String> {
    let mut args = parse_command_line(command_line)?;
    if args.is_empty() {
        return Err("Command is empty.".to_string());
    }
    if matches!(
        args.first().map(|s| s.as_str()),
        Some("microcodes" | "mcodes")
    ) {
        args.remove(0);
    }
    if args.is_empty() {
        return Err("No subcommand provided.".to_string());
    }

    let exe =
        std::env::current_exe().map_err(|e| format!("Failed to resolve executable: {}", e))?;
    let mut cmd = Command::new(exe);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("MICROCODES_BASE_URL", &api.base_url);
    cmd.stdin(Stdio::null());
    if let Some(token) = &api.token {
        cmd.env("MICROCODES_API_TOKEN", token);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run command: {}", e))?;
    let mut merged = String::new();
    if !output.stdout.is_empty() {
        merged.push_str(&String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        if !merged.is_empty() && !merged.ends_with('\n') {
            merged.push('\n');
        }
        merged.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    if merged.trim().is_empty() {
        merged = if output.status.success() {
            "(command completed with no output)".to_string()
        } else {
            "(command failed with no output)".to_string()
        };
    }

    Ok(CommandRunResult {
        command: command_line.to_string(),
        output: merged,
        exit_code: output.status.code().unwrap_or(-1),
    })
}

enum ApiMsg {
    WhoAmI(Result<Value, String>),
    SearchResults(Result<Value, String>),
    SnippetDetail(Result<Value, String>),
    SnippetMetrics(Result<Value, String>),
    SnippetVersions(Result<Value, String>),
    SnippetAction(Result<String, String>),
    SnippetSubmitted(Result<String, String>),
    SnippetPatched(Result<(), String>),
    MyLists(Result<Value, String>),
    ListDetail(Result<Value, String>),
    ListAction(Result<String, String>),
    RequestsList(Result<Value, String>),
    RequestDetail(Result<Value, String>),
    RequestAction(Result<String, String>),
    CommentsList(Result<Value, String>),
    CommentAction(Result<String, String>),
    CliCommand(Result<CommandRunResult, String>),
}

struct CommandRunResult {
    command: String,
    output: String,
    exit_code: i32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Snippets,
    Lists,
    Requests,
    Commands,
}

const TABS: &[Tab] = &[
    Tab::Overview,
    Tab::Snippets,
    Tab::Lists,
    Tab::Requests,
    Tab::Commands,
];

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Snippets => "Snippets",
            Tab::Lists => "Lists",
            Tab::Requests => "Requests",
            Tab::Commands => "Commands",
        }
    }
}

#[derive(Clone, PartialEq)]
enum SnippetsMode {
    Browse,
    Detail { scroll: u16 },
    Actions,
    Metrics,
    Versions,
    SubmitPicker,
    SubmitFile,
    SubmitFolder,
    SubmitStdin,
    SubmitForm,
    PatchForm,
    FiltersForm,
    SortPicker,
    Confirm(ConfirmKind),
    AddToList,
    Comments,
}

#[derive(Clone, PartialEq)]
enum ListsMode {
    Browse,
    Detail,
    CreateForm,
    EditForm,
    AddSnippet,
    Confirm(ConfirmKind),
}

#[derive(Clone, PartialEq)]
enum RequestsMode {
    Browse,
    Detail,
    SubmitForm,
    Confirm(ConfirmKind),
}

#[derive(Clone, PartialEq)]
enum OverviewMode {
    Profile,
}

#[derive(Clone, PartialEq)]
enum ConfirmKind {
    DeleteSnippet(String),
    DeleteList(String),
    DeleteRequest(String),
    DeleteComment(String),
}

fn blank_area<'a>(title: &'a str) -> TextArea<'a> {
    let mut ta = TextArea::default();
    ta.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(teal())
            .title(Span::styled(title, teal())),
    );
    ta.set_style(Style::default().fg(FG));
    ta.set_cursor_line_style(Style::default());
    ta
}

fn focused_area<'a>(title: &'a str) -> TextArea<'a> {
    let mut ta = blank_area(title);
    ta.set_block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(bold_teal())
            .title(Span::styled(title, bold_teal())),
    );
    ta
}

struct SnippetForm {
    fields: Vec<TextArea<'static>>,
    focus: usize,
}

impl SnippetForm {
    fn new() -> Self {
        let names = [
            "Title *",
            "Language *",
            "Description",
            "Tags (comma-sep)",
            "Code *",
        ];
        let fields: Vec<TextArea<'static>> = names.iter().map(|n| blank_area(n)).collect();
        let mut s = Self { fields, focus: 0 };
        s.refresh_styles();
        s
    }

    fn new_prefilled(title: &str, description: &str, language: &str, tags: &str) -> Self {
        let mut s = Self::new();
        s.fields[0].insert_str(title);
        s.fields[1].insert_str(language);
        s.fields[2].insert_str(description);
        s.fields[3].insert_str(tags);
        s
    }

    fn refresh_styles(&mut self) {
        let names = [
            "Title *",
            "Language *",
            "Description",
            "Tags (comma-sep)",
            "Code *",
        ];
        for (i, f) in self.fields.iter_mut().enumerate() {
            let style = if i == self.focus { bold_teal() } else { teal() };
            let name = names[i];
            f.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(style)
                    .title(Span::styled(name, style)),
            );
        }
    }

    fn next_field(&mut self) {
        self.focus = (self.focus + 1) % self.fields.len();
        self.refresh_styles();
    }

    fn prev_field(&mut self) {
        if self.focus == 0 {
            self.focus = self.fields.len() - 1;
        } else {
            self.focus -= 1;
        }
        self.refresh_styles();
    }

    fn value(&self, i: usize) -> String {
        self.fields[i].lines().join("\n")
    }

    fn to_json(&self) -> Result<Value, String> {
        let title = self.value(0);
        let language = self.value(1);
        let code = self.value(4);
        if title.trim().is_empty() {
            return Err("Title is required".to_string());
        }
        if language.trim().is_empty() {
            return Err("Language is required".to_string());
        }
        if code.trim().is_empty() {
            return Err("Code is required".to_string());
        }
        let tags: Vec<Value> = self
            .value(3)
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .map(Value::String)
            .collect();
        Ok(json!({
            "title": title.trim(),
            "language": language.trim(),
            "description": self.value(2).trim().to_string(),
            "tags": tags,
            "code": code,
        }))
    }
}

struct SnippetFiltersForm {
    fields: Vec<TextArea<'static>>,
    focus: usize,
}

impl SnippetFiltersForm {
    fn new() -> Self {
        let names = [
            "Tags (comma-sep)",
            "Languages (comma-sep)",
            "Submitter",
            "AI Generated (include|exclude|only)",
            "My snippets only (yes|no)",
        ];
        let fields: Vec<TextArea<'static>> = names.iter().map(|n| blank_area(n)).collect();
        let mut s = Self { fields, focus: 0 };
        s.refresh_styles();
        s
    }

    fn new_prefilled(
        tags: Option<&str>,
        languages: Option<&str>,
        submitter: Option<&str>,
        generated: Option<&str>,
        mine_only: bool,
    ) -> Self {
        let mut s = Self::new();
        if let Some(v) = tags {
            s.fields[0].insert_str(v);
        }
        if let Some(v) = languages {
            s.fields[1].insert_str(v);
        }
        if let Some(v) = submitter {
            s.fields[2].insert_str(v);
        }
        if let Some(v) = generated {
            s.fields[3].insert_str(v);
        }
        s.fields[4].insert_str(if mine_only { "yes" } else { "no" });
        s
    }

    fn refresh_styles(&mut self) {
        let names = [
            "Tags (comma-sep)",
            "Languages (comma-sep)",
            "Submitter",
            "AI Generated (include|exclude|only)",
            "My snippets only (yes|no)",
        ];
        for (i, f) in self.fields.iter_mut().enumerate() {
            let style = if i == self.focus { bold_teal() } else { teal() };
            f.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(style)
                    .title(Span::styled(names[i], style)),
            );
        }
    }

    fn next_field(&mut self) {
        self.focus = (self.focus + 1) % self.fields.len();
        self.refresh_styles();
    }

    fn prev_field(&mut self) {
        if self.focus == 0 {
            self.focus = self.fields.len() - 1;
        } else {
            self.focus -= 1;
        }
        self.refresh_styles();
    }

    fn value(&self, i: usize) -> String {
        self.fields[i].lines().join(" ").trim().to_string()
    }
}

struct ListForm {
    fields: Vec<TextArea<'static>>,
    focus: usize,
    unlisted: bool,
}

impl ListForm {
    fn new() -> Self {
        let names = ["Title *", "Description"];
        let fields: Vec<TextArea<'static>> = names.iter().map(|n| blank_area(n)).collect();
        let mut s = Self {
            fields,
            focus: 0,
            unlisted: false,
        };
        s.refresh_styles();
        s
    }

    fn new_prefilled(title: &str, description: &str, unlisted: bool) -> Self {
        let mut s = Self::new();
        s.fields[0].insert_str(title);
        s.fields[1].insert_str(description);
        s.unlisted = unlisted;
        s
    }

    fn refresh_styles(&mut self) {
        let names = ["Title *", "Description"];
        for (i, f) in self.fields.iter_mut().enumerate() {
            let style = if i == self.focus { bold_teal() } else { teal() };
            f.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(style)
                    .title(Span::styled(names[i], style)),
            );
        }
    }

    fn next_field(&mut self) {
        self.focus = (self.focus + 1) % self.fields.len();
        self.refresh_styles();
    }

    fn value(&self, i: usize) -> String {
        self.fields[i].lines().join("\n")
    }
}

struct RequestForm {
    fields: Vec<TextArea<'static>>,
    focus: usize,
}

impl RequestForm {
    fn new() -> Self {
        let names = ["Title *", "Description *", "Tags (comma-sep)"];
        let fields: Vec<TextArea<'static>> = names.iter().map(|n| blank_area(n)).collect();
        let mut s = Self { fields, focus: 0 };
        s.refresh_styles();
        s
    }

    fn refresh_styles(&mut self) {
        let names = ["Title *", "Description *", "Tags (comma-sep)"];
        for (i, f) in self.fields.iter_mut().enumerate() {
            let style = if i == self.focus { bold_teal() } else { teal() };
            f.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(style)
                    .title(Span::styled(names[i], style)),
            );
        }
    }

    fn next_field(&mut self) {
        self.focus = (self.focus + 1) % self.fields.len();
        self.refresh_styles();
    }

    fn value(&self, i: usize) -> String {
        self.fields[i].lines().join("\n")
    }
}

struct SingleInput {
    inner: TextArea<'static>,
}

impl SingleInput {
    fn new(title: &'static str) -> Self {
        let mut ta = blank_area(title);
        Self { inner: ta }
    }
    fn value(&self) -> String {
        self.inner.lines().first().cloned().unwrap_or_default()
    }
}

struct OverviewState {
    mode: OverviewMode,
    profile: Option<Value>,
    loading: bool,
    error: Option<String>,
}

impl OverviewState {
    fn new() -> Self {
        Self {
            mode: OverviewMode::Profile,
            profile: None,
            loading: false,
            error: None,
        }
    }
}

const SNIPPET_ACTIONS: &[&str] = &[
    "View Full Detail",
    "Vote Up",
    "Vote Down",
    "Remove Vote",
    "Bookmark",
    "Remove Bookmark",
    "View Metrics",
    "View Versions",
    "Add to List",
    "Edit (Patch)",
    "Delete",
];

const SNIPPET_SORT_OPTIONS: &[&str] = &["relevance", "oldest", "newest", "upvotes"];
const SNIPPET_SUBMIT_SOURCES: &[&str] = &["File", "Folder", "Stdin", "Form"];

struct SnippetsState {
    mode: SnippetsMode,
    search_input: TextArea<'static>,
    search_active: bool,
    results: Vec<Value>,
    table_state: TableState,
    detail: Option<Value>,
    metrics: Option<Value>,
    versions: Vec<Value>,
    versions_state: TableState,
    action_state: ListState,
    submit_source_state: ListState,
    sort_state: ListState,
    submit_form: SnippetForm,
    submit_file_input: TextArea<'static>,
    submit_folder_input: TextArea<'static>,
    submit_stdin_input: TextArea<'static>,
    patch_form: SnippetForm,
    filters_form: SnippetFiltersForm,
    add_to_list_input: TextArea<'static>,
    filter_tags: Option<String>,
    filter_languages: Option<String>,
    filter_submitter: Option<String>,
    filter_generated: Option<String>,
    filter_mine_only: bool,
    sort: Option<String>,
    page: u32,
    loading: bool,
    error: Option<String>,
    success: Option<String>,
    comments: Vec<Value>,
    comments_state: TableState,
    comment_input: TextArea<'static>,
}

impl SnippetsState {
    fn new() -> Self {
        let mut search_input = blank_area("Search (press / to focus)");
        search_input.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(teal())
                .title(Span::styled("Search  (press / to focus)", teal())),
        );
        let mut action_state = ListState::default();
        action_state.select(Some(0));
        let mut submit_source_state = ListState::default();
        submit_source_state.select(Some(0));
        let mut sort_state = ListState::default();
        sort_state.select(Some(0));
        Self {
            mode: SnippetsMode::Browse,
            search_input,
            search_active: false,
            results: vec![],
            table_state: TableState::default(),
            detail: None,
            metrics: None,
            versions: vec![],
            versions_state: TableState::default(),
            action_state,
            submit_source_state,
            sort_state,
            submit_form: SnippetForm::new(),
            submit_file_input: blank_area("File path (Enter submit, Esc cancel)"),
            submit_folder_input: blank_area("Folder path (Enter submit, Esc cancel)"),
            submit_stdin_input: blank_area("Snippet JSON payload (F10 submit, Esc cancel)"),
            patch_form: SnippetForm::new(),
            filters_form: SnippetFiltersForm::new(),
            add_to_list_input: blank_area("List ID"),
            filter_tags: None,
            filter_languages: None,
            filter_submitter: None,
            filter_generated: None,
            filter_mine_only: false,
            sort: Some("relevance".to_string()),
            page: 1,
            loading: false,
            error: None,
            success: None,
            comments: vec![],
            comments_state: TableState::default(),
            comment_input: blank_area("Comment text (Enter to submit, Esc to cancel)"),
        }
    }

    fn selected_id(&self) -> Option<String> {
        let i = self.table_state.selected()?;
        self.results.get(i).map(|v| sv(v, "id"))
    }
}

struct ListsState {
    mode: ListsMode,
    my_lists: Vec<Value>,
    table_state: TableState,
    detail: Option<Value>,
    detail_table: TableState,
    create_form: ListForm,
    edit_form: ListForm,
    add_snippet_input: TextArea<'static>,
    loading: bool,
    error: Option<String>,
    success: Option<String>,
}

impl ListsState {
    fn new() -> Self {
        Self {
            mode: ListsMode::Browse,
            my_lists: vec![],
            table_state: TableState::default(),
            detail: None,
            detail_table: TableState::default(),
            create_form: ListForm::new(),
            edit_form: ListForm::new(),
            add_snippet_input: blank_area("Snippet ID"),
            loading: false,
            error: None,
            success: None,
        }
    }

    fn selected_id(&self) -> Option<String> {
        let i = self.table_state.selected()?;
        self.my_lists.get(i).map(|v| sv(v, "id"))
    }
}

struct RequestsState {
    mode: RequestsMode,
    requests: Vec<Value>,
    table_state: TableState,
    detail: Option<Value>,
    submit_form: RequestForm,
    loading: bool,
    error: Option<String>,
    success: Option<String>,
}

impl RequestsState {
    fn new() -> Self {
        Self {
            mode: RequestsMode::Browse,
            requests: vec![],
            table_state: TableState::default(),
            detail: None,
            submit_form: RequestForm::new(),
            loading: false,
            error: None,
            success: None,
        }
    }

    fn selected_id(&self) -> Option<String> {
        let i = self.table_state.selected()?;
        self.requests.get(i).map(|v| sv(v, "id"))
    }
}

struct CommandsState {
    input: TextArea<'static>,
    input_active: bool,
    output: String,
    scroll: u16,
    loading: bool,
    error: Option<String>,
    success: Option<String>,
    history: Vec<String>,
    history_cursor: Option<usize>,
    last_command: Option<String>,
}

impl CommandsState {
    fn new() -> Self {
        let mut s = Self {
            input: focused_area("Command (Enter to run, Esc to blur)"),
            input_active: true,
            output: String::new(),
            scroll: 0,
            loading: false,
            error: None,
            success: None,
            history: vec![],
            history_cursor: None,
            last_command: None,
        };
        s.input.insert_str("help");
        s
    }

    fn command_text(&self) -> String {
        self.input.lines().join(" ").trim().to_string()
    }

    fn set_input_text(&mut self, text: &str) {
        let title = if self.input_active {
            "Command (Enter to run, Esc to blur)"
        } else {
            "Command (press / or i to focus)"
        };
        let mut ta = if self.input_active {
            focused_area(title)
        } else {
            blank_area(title)
        };
        ta.insert_str(text);
        self.input = ta;
    }

    fn set_input_active(&mut self, active: bool) {
        self.input_active = active;
        let existing = self.command_text();
        self.set_input_text(&existing);
    }
}

struct App {
    tab: Tab,
    overview: OverviewState,
    snippets: SnippetsState,
    lists: ListsState,
    requests: RequestsState,
    commands: CommandsState,
    api: ApiConfig,
    tx: SyncSender<ApiMsg>,
    rx: Receiver<ApiMsg>,
    should_quit: bool,
    show_help: bool,
}

impl App {
    fn new(api: ApiConfig) -> Self {
        let (tx, rx) = mpsc::sync_channel(64);
        let mut app = Self {
            tab: Tab::Overview,
            overview: OverviewState::new(),
            snippets: SnippetsState::new(),
            lists: ListsState::new(),
            requests: RequestsState::new(),
            commands: CommandsState::new(),
            api,
            tx,
            rx,
            should_quit: false,
            show_help: false,
        };
        app.load_overview();
        app
    }

    fn load_overview(&mut self) {
        if self.api.token.is_none() {
            return;
        }
        self.overview.loading = true;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api.auth_get("/api/me");
            let _ = tx.send(ApiMsg::WhoAmI(r));
        });
    }

    fn do_search(&mut self) {
        let q = self.snippets.search_input.lines().join(" ");
        let q_trimmed = q.trim().to_string();
        let has_filters = self.snippets.filter_tags.is_some()
            || self.snippets.filter_languages.is_some()
            || self.snippets.filter_submitter.is_some()
            || self.snippets.filter_generated.is_some()
            || self.snippets.filter_mine_only;
        if q_trimmed.is_empty() && !has_filters {
            self.snippets.error = Some("Enter a search query or set filters first.".to_string());
            return;
        }
        self.snippets.loading = true;
        self.snippets.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        let tags = self.snippets.filter_tags.clone();
        let languages = self.snippets.filter_languages.clone();
        let submitter = self.snippets.filter_submitter.clone();
        let mine_only = self.snippets.filter_mine_only;
        let generated = self.snippets.filter_generated.clone();
        let sort = self.snippets.sort.clone();
        let page = self.snippets.page.to_string();
        let q_for_thread = if q_trimmed.is_empty() { "*".to_string() } else { q_trimmed.clone() };
        thread::spawn(move || {
            let r = if let Some(ids) = q_for_thread.strip_prefix("id:") {
                let ids = ids.trim();
                if ids.is_empty() {
                    Err("Use id:<comma-separated-ids> to fetch by IDs.".to_string())
                } else {
                    api.get_q("/api/snippets/by-ids", &[("ids", ids)])
                }
            } else {
                let mut params: Vec<(&str, &str)> = vec![];
                if q_for_thread != "*" {
                    params.push(("q", q_for_thread.as_str()));
                }
                if let Some(v) = tags.as_deref() {
                    if !v.trim().is_empty() {
                        params.push(("tags", v));
                    }
                }
                if let Some(v) = languages.as_deref() {
                    if !v.trim().is_empty() {
                        params.push(("languages", v));
                    }
                }
                if mine_only {
                    params.push(("submitter", "me"));
                } else if let Some(v) = submitter.as_deref() {
                    if !v.trim().is_empty() {
                        params.push(("submitter", v));
                    }
                }
                if let Some(v) = generated.as_deref() {
                    if !v.trim().is_empty() {
                        params.push(("generated", v));
                    }
                }
                if let Some(v) = sort.as_deref() {
                    if !v.trim().is_empty() {
                        params.push(("sort", v));
                    }
                }
                params.push(("page", page.as_str()));
                api.get_q("/api/snippets", &params)
            };
            let _ = tx.send(ApiMsg::SearchResults(r));
        });
    }

    fn load_snippet_detail(&mut self, id: String) {
        self.snippets.loading = true;
        self.snippets.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api.get(&format!("/api/snippets/{}", id));
            let _ = tx.send(ApiMsg::SnippetDetail(r));
        });
    }

    fn load_snippet_metrics(&mut self, id: String) {
        self.snippets.loading = true;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api.get(&format!("/api/snippets/{}/metrics", id));
            let _ = tx.send(ApiMsg::SnippetMetrics(r));
        });
    }

    fn load_snippet_versions(&mut self, id: String) {
        self.snippets.loading = true;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api.auth_get(&format!("/api/snippets/{}/versions", id));
            let _ = tx.send(ApiMsg::SnippetVersions(r));
        });
    }

    fn do_vote(&mut self, id: String, dir: &str) {
        self.snippets.loading = true;
        let api = self.api.clone();
        let tx = self.tx.clone();
        let dir = dir.to_string();
        thread::spawn(move || {
            let r = api
                .auth_put(&format!("/api/snippets/{}/vote", id), json!({"value": dir}))
                .map(|_| format!("Vote '{}' applied.", dir));
            let _ = tx.send(ApiMsg::SnippetAction(r));
        });
    }

    fn do_bookmark(&mut self, id: String, add: bool) {
        self.snippets.loading = true;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api
                .auth_put(
                    &format!("/api/snippets/{}/bookmark", id),
                    json!({"value": add}),
                )
                .map(|_| {
                    if add {
                        "Bookmarked.".to_string()
                    } else {
                        "Bookmark removed.".to_string()
                    }
                });
            let _ = tx.send(ApiMsg::SnippetAction(r));
        });
    }

    fn do_delete_snippet(&mut self, id: String) {
        self.snippets.loading = true;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api
                .auth_delete(&format!("/api/snippets/{}", id))
                .map(|_| "Snippet deleted.".to_string());
            let _ = tx.send(ApiMsg::SnippetAction(r));
        });
    }

    fn do_submit_snippet(&mut self) {
        match self.snippets.submit_form.to_json() {
            Err(e) => {
                self.snippets.error = Some(e);
            }
            Ok(body) => self.do_submit_payload(body),
        }
    }

    fn do_submit_payload(&mut self, body: Value) {
        self.snippets.loading = true;
        self.snippets.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api
                .auth_post("/api/snippets", body)
                .map(|v| format!("Snippet created: {}", sv(&v, "id")));
            let _ = tx.send(ApiMsg::SnippetSubmitted(r));
        });
    }

    fn do_submit_file(&mut self) {
        let path = self
            .snippets
            .submit_file_input
            .lines()
            .first()
            .cloned()
            .unwrap_or_default()
            .trim()
            .to_string();
        if path.is_empty() {
            self.snippets.error = Some("File path is required.".to_string());
            return;
        }
        self.snippets.loading = true;
        self.snippets.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = (|| {
                let raw = std::fs::read_to_string(&path)
                    .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;
                let body: Value =
                    serde_json::from_str(&raw).map_err(|e| format!("Invalid JSON: {}", e))?;
                api.auth_post("/api/snippets", body)
                    .map(|v| format!("Snippet created: {}", sv(&v, "id")))
            })();
            let _ = tx.send(ApiMsg::SnippetSubmitted(r));
        });
    }

    fn do_submit_folder(&mut self) {
        let path = self
            .snippets
            .submit_folder_input
            .lines()
            .first()
            .cloned()
            .unwrap_or_default()
            .trim()
            .to_string();
        if path.is_empty() {
            self.snippets.error = Some("Folder path is required.".to_string());
            return;
        }
        self.snippets.loading = true;
        self.snippets.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = (|| {
                let body = build_folder_payload(&path)?;
                api.auth_post("/api/snippets", body)
                    .map(|v| format!("Snippet created: {}", sv(&v, "id")))
            })();
            let _ = tx.send(ApiMsg::SnippetSubmitted(r));
        });
    }

    fn do_submit_stdin_json(&mut self) {
        let raw = self.snippets.submit_stdin_input.lines().join("\n");
        if raw.trim().is_empty() {
            self.snippets.error = Some("JSON payload is required.".to_string());
            return;
        }
        match serde_json::from_str::<Value>(&raw) {
            Err(e) => self.snippets.error = Some(format!("Invalid JSON: {}", e)),
            Ok(body) => self.do_submit_payload(body),
        }
    }

    fn do_patch_snippet(&mut self, id: String) {
        let mut ops: Vec<Value> = vec![];
        let title = self.snippets.patch_form.value(0);
        let language = self.snippets.patch_form.value(1);
        let description = self.snippets.patch_form.value(2);
        if !title.trim().is_empty() {
            ops.push(json!({"op":"replace","path":"/title","value": title.trim()}));
        }
        if !language.trim().is_empty() {
            ops.push(json!({"op":"replace","path":"/language","value": language.trim()}));
        }
        if !description.trim().is_empty() {
            ops.push(json!({"op":"replace","path":"/description","value": description.trim()}));
        }
        if ops.is_empty() {
            self.snippets.error = Some("No fields to patch.".to_string());
            return;
        }
        self.snippets.loading = true;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api
                .auth_patch(&format!("/api/snippets/{}", id), Value::Array(ops))
                .map(|_| "Snippet updated.".to_string());
            let _ = tx.send(ApiMsg::SnippetAction(r));
        });
    }

    fn do_add_to_list(&mut self, list_id: String) {
        let snippet_id = match self.snippets.selected_id() {
            Some(id) => id,
            None => return,
        };
        self.snippets.loading = true;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api
                .auth_post(
                    &format!("/api/lists/{}/snippets", list_id),
                    json!({"snippetId": snippet_id}),
                )
                .map(|_| "Added to list.".to_string());
            let _ = tx.send(ApiMsg::SnippetAction(r));
        });
    }

    fn load_my_lists(&mut self) {
        self.lists.loading = true;
        self.lists.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api.auth_get("/api/lists");
            let _ = tx.send(ApiMsg::MyLists(r));
        });
    }

    fn load_list_detail(&mut self, id: String) {
        self.lists.loading = true;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api.get(&format!("/api/lists/{}", id));
            let _ = tx.send(ApiMsg::ListDetail(r));
        });
    }

    fn do_create_list(&mut self) {
        let title = self.lists.create_form.value(0);
        if title.trim().is_empty() {
            self.lists.error = Some("Title is required.".to_string());
            return;
        }
        let description = self.lists.create_form.value(1);
        let unlisted = self.lists.create_form.unlisted;
        self.lists.loading = true;
        self.lists.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api
                .auth_post(
                    "/api/lists",
                    json!({
                        "title": title.trim(),
                        "description": description.trim(),
                        "unlisted": unlisted,
                    }),
                )
                .map(|v| format!("List created: {}", sv(&v, "id")));
            let _ = tx.send(ApiMsg::ListAction(r));
        });
    }

    fn do_delete_list(&mut self, id: String) {
        self.lists.loading = true;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api
                .auth_delete(&format!("/api/lists/{}", id))
                .map(|_| "List deleted.".to_string());
            let _ = tx.send(ApiMsg::ListAction(r));
        });
    }

    fn do_add_snippet_to_list(&mut self) {
        let snippet_id = self
            .lists
            .add_snippet_input
            .lines()
            .first()
            .cloned()
            .unwrap_or_default();
        if snippet_id.trim().is_empty() {
            self.lists.error = Some("Snippet ID required.".to_string());
            return;
        }
        let list_id = match self.lists.selected_id() {
            Some(id) => id,
            None => return,
        };
        self.lists.loading = true;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api
                .auth_post(
                    &format!("/api/lists/{}/snippets", list_id),
                    json!({"snippetId": snippet_id.trim()}),
                )
                .map(|_| "Snippet added to list.".to_string());
            let _ = tx.send(ApiMsg::ListAction(r));
        });
    }

    fn load_requests(&mut self) {
        self.requests.loading = true;
        self.requests.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api.get("/api/requests");
            let _ = tx.send(ApiMsg::RequestsList(r));
        });
    }

    fn load_request_detail(&mut self, id: String) {
        self.requests.loading = true;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api.get(&format!("/api/requests/{}", id));
            let _ = tx.send(ApiMsg::RequestDetail(r));
        });
    }

    fn do_submit_request(&mut self) {
        let title = self.requests.submit_form.value(0);
        let description = self.requests.submit_form.value(1);
        if title.trim().is_empty() {
            self.requests.error = Some("Title is required.".to_string());
            return;
        }
        if description.trim().is_empty() {
            self.requests.error = Some("Description is required.".to_string());
            return;
        }
        let tags: Vec<Value> = self
            .requests
            .submit_form
            .value(2)
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .map(Value::String)
            .collect();
        self.requests.loading = true;
        self.requests.error = None;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api
                .auth_post(
                    "/api/requests",
                    json!({
                        "title": title.trim(),
                        "description": description.trim(),
                        "tags": tags,
                    }),
                )
                .map(|v| format!("Request submitted: {}", sv(&v, "id")));
            let _ = tx.send(ApiMsg::RequestAction(r));
        });
    }

    fn do_delete_request(&mut self, id: String) {
        self.requests.loading = true;
        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = api
                .auth_delete(&format!("/api/requests/{}", id))
                .map(|_| "Request deleted.".to_string());
            let _ = tx.send(ApiMsg::RequestAction(r));
        });
    }

    fn run_cli_command(&mut self, command_line: String) {
        if command_line.trim().is_empty() {
            self.commands.error = Some("Command is empty.".to_string());
            return;
        }
        self.commands.loading = true;
        self.commands.error = None;
        self.commands.success = None;
        self.commands.last_command = Some(command_line.clone());
        if self
            .commands
            .history
            .last()
            .map(|c| c != &command_line)
            .unwrap_or(true)
        {
            self.commands.history.push(command_line.clone());
        }
        self.commands.history_cursor = None;

        let api = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let r = execute_cli_command(&command_line, &api);
            let _ = tx.send(ApiMsg::CliCommand(r));
        });
    }

    fn poll_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                ApiMsg::WhoAmI(r) => {
                    self.overview.loading = false;
                    match r {
                        Ok(v) => self.overview.profile = Some(v),
                        Err(e) => self.overview.error = Some(e),
                    }
                }
                ApiMsg::SearchResults(r) => {
                    self.snippets.loading = false;
                    match r {
                        Ok(v) => {
                            self.snippets.results = v.as_array().cloned().unwrap_or_default();
                            if !self.snippets.results.is_empty() {
                                self.snippets.table_state.select(Some(0));
                            }
                        }
                        Err(e) => self.snippets.error = Some(e),
                    }
                }
                ApiMsg::SnippetDetail(r) => {
                    self.snippets.loading = false;
                    match r {
                        Ok(v) => {
                            self.snippets.detail = Some(v);
                            self.snippets.mode = SnippetsMode::Detail { scroll: 0 };
                        }
                        Err(e) => self.snippets.error = Some(e),
                    }
                }
                ApiMsg::SnippetMetrics(r) => {
                    self.snippets.loading = false;
                    match r {
                        Ok(v) => {
                            self.snippets.metrics = Some(v);
                            self.snippets.mode = SnippetsMode::Metrics;
                        }
                        Err(e) => self.snippets.error = Some(e),
                    }
                }
                ApiMsg::SnippetVersions(r) => {
                    self.snippets.loading = false;
                    match r {
                        Ok(v) => {
                            self.snippets.versions = v.as_array().cloned().unwrap_or_default();
                            self.snippets.mode = SnippetsMode::Versions;
                        }
                        Err(e) => self.snippets.error = Some(e),
                    }
                }
                ApiMsg::SnippetAction(r) => {
                    self.snippets.loading = false;
                    match r {
                        Ok(msg) => {
                            self.snippets.success = Some(msg);
                            self.snippets.mode = SnippetsMode::Browse;
                        }
                        Err(e) => self.snippets.error = Some(e),
                    }
                }
                ApiMsg::SnippetSubmitted(r) => {
                    self.snippets.loading = false;
                    match r {
                        Ok(msg) => {
                            self.snippets.success = Some(msg);
                            self.snippets.mode = SnippetsMode::Browse;
                            self.snippets.submit_source_state.select(Some(0));
                            self.snippets.submit_form = SnippetForm::new();
                            self.snippets.submit_file_input =
                                blank_area("File path (Enter submit, Esc cancel)");
                            self.snippets.submit_folder_input =
                                blank_area("Folder path (Enter submit, Esc cancel)");
                            self.snippets.submit_stdin_input =
                                blank_area("Snippet JSON payload (F10 submit, Esc cancel)");
                        }
                        Err(e) => self.snippets.error = Some(e),
                    }
                }
                ApiMsg::SnippetPatched(r) => {
                    self.snippets.loading = false;
                    match r {
                        Ok(()) => {
                            self.snippets.success = Some("Snippet patched.".to_string());
                            self.snippets.mode = SnippetsMode::Browse;
                        }
                        Err(e) => self.snippets.error = Some(e),
                    }
                }
                ApiMsg::MyLists(r) => {
                    self.lists.loading = false;
                    match r {
                        Ok(v) => {
                            self.lists.my_lists = v.as_array().cloned().unwrap_or_default();
                            if !self.lists.my_lists.is_empty() {
                                self.lists.table_state.select(Some(0));
                            }
                        }
                        Err(e) => self.lists.error = Some(e),
                    }
                }
                ApiMsg::ListDetail(r) => {
                    self.lists.loading = false;
                    match r {
                        Ok(v) => {
                            self.lists.detail = Some(v);
                            self.lists.mode = ListsMode::Detail;
                        }
                        Err(e) => self.lists.error = Some(e),
                    }
                }
                ApiMsg::ListAction(r) => {
                    self.lists.loading = false;
                    match r {
                        Ok(msg) => {
                            self.lists.success = Some(msg);
                            self.lists.mode = ListsMode::Browse;
                            self.load_my_lists();
                        }
                        Err(e) => self.lists.error = Some(e),
                    }
                }
                ApiMsg::RequestsList(r) => {
                    self.requests.loading = false;
                    match r {
                        Ok(v) => {
                            self.requests.requests = v.as_array().cloned().unwrap_or_default();
                            if !self.requests.requests.is_empty() {
                                self.requests.table_state.select(Some(0));
                            }
                        }
                        Err(e) => self.requests.error = Some(e),
                    }
                }
                ApiMsg::RequestDetail(r) => {
                    self.requests.loading = false;
                    match r {
                        Ok(v) => {
                            self.requests.detail = Some(v);
                            self.requests.mode = RequestsMode::Detail;
                        }
                        Err(e) => self.requests.error = Some(e),
                    }
                }
                ApiMsg::RequestAction(r) => {
                    self.requests.loading = false;
                    match r {
                        Ok(msg) => {
                            self.requests.success = Some(msg);
                            self.requests.mode = RequestsMode::Browse;
                            self.load_requests();
                        }
                        Err(e) => self.requests.error = Some(e),
                    }
                }
                ApiMsg::CommentsList(r) => {
                    self.snippets.loading = false;
                    match r {
                        Ok(v) => {
                            self.snippets.comments = v.as_array().cloned().unwrap_or_default();
                            self.snippets.mode = SnippetsMode::Comments;
                        }
                        Err(e) => self.snippets.error = Some(e),
                    }
                }
                ApiMsg::CommentAction(r) => {
                    self.snippets.loading = false;
                    match r {
                        Ok(msg) => {
                            self.snippets.success = Some(msg);
                            self.snippets.mode = SnippetsMode::Browse;
                        }
                        Err(e) => self.snippets.error = Some(e),
                    }
                }
                ApiMsg::CliCommand(r) => {
                    self.commands.loading = false;
                    match r {
                        Ok(result) => {
                            self.commands.output = result.output;
                            self.commands.scroll = 0;
                            if result.exit_code == 0 {
                                self.commands.success =
                                    Some(format!("Command succeeded: {}", result.command));
                                self.commands.error = None;
                            } else {
                                self.commands.error = Some(format!(
                                    "Command failed (exit code {}): {}",
                                    result.exit_code, result.command
                                ));
                                self.commands.success = None;
                            }
                        }
                        Err(e) => {
                            self.commands.error = Some(e);
                            self.commands.success = None;
                        }
                    }
                }
            }
        }
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        if key.code == KeyCode::Char('?') && key.modifiers == KeyModifiers::NONE {
            self.show_help = !self.show_help;
            return;
        }
        if self.show_help {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('q') {
                self.show_help = false;
            }
            return;
        }

        let in_input = self.is_in_input_mode();
        if !in_input {
            match key.code {
                KeyCode::Char('1') => {
                    self.switch_tab(Tab::Overview);
                    return;
                }
                KeyCode::Char('2') => {
                    self.switch_tab(Tab::Snippets);
                    return;
                }
                KeyCode::Char('3') => {
                    self.switch_tab(Tab::Lists);
                    return;
                }
                KeyCode::Char('4') => {
                    self.switch_tab(Tab::Requests);
                    return;
                }
                KeyCode::Char('5') => {
                    self.switch_tab(Tab::Commands);
                    return;
                }
                _ => {}
            }
        }

        match self.tab {
            Tab::Overview => self.handle_overview_key(key),
            Tab::Snippets => self.handle_snippets_key(key),
            Tab::Lists => self.handle_lists_key(key),
            Tab::Requests => self.handle_requests_key(key),
            Tab::Commands => self.handle_commands_key(key),
        }
    }

    fn is_in_input_mode(&self) -> bool {
        match self.tab {
            Tab::Snippets => {
                matches!(
                    self.snippets.mode,
                    SnippetsMode::SubmitPicker
                        | SnippetsMode::SubmitFile
                        | SnippetsMode::SubmitFolder
                        | SnippetsMode::SubmitStdin
                        | SnippetsMode::SubmitForm
                        | SnippetsMode::PatchForm
                        | SnippetsMode::FiltersForm
                        | SnippetsMode::AddToList
                ) || self.snippets.search_active
            }
            Tab::Lists => matches!(
                self.lists.mode,
                ListsMode::CreateForm | ListsMode::EditForm | ListsMode::AddSnippet
            ),
            Tab::Requests => matches!(self.requests.mode, RequestsMode::SubmitForm),
            Tab::Commands => self.commands.input_active,
            _ => false,
        }
    }

    fn switch_tab(&mut self, tab: Tab) {
        self.tab = tab;
        match tab {
            Tab::Overview => {
                if self.overview.profile.is_none() && !self.overview.loading {
                    self.load_overview();
                }
            }
            Tab::Lists => {
                if self.lists.my_lists.is_empty() && !self.lists.loading {
                    self.load_my_lists();
                }
            }
            Tab::Requests => {
                if self.requests.requests.is_empty() && !self.requests.loading {
                    self.load_requests();
                }
            }
            _ => {}
        }
    }

    fn handle_overview_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char('r') => {
                self.overview.profile = None;
                self.load_overview();
            }
            KeyCode::Char('q') => self.should_quit = true,
            _ => {}
        }
    }

    fn handle_snippets_key(&mut self, key: crossterm::event::KeyEvent) {
        self.snippets.success = None;

        let mode = self.snippets.mode.clone();
        match mode {
            SnippetsMode::Browse => self.snippets_browse_key(key),
            SnippetsMode::Detail { scroll } => self.snippets_detail_key(key, scroll),
            SnippetsMode::Actions => self.snippets_actions_key(key),
            SnippetsMode::Metrics => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                    self.snippets.mode = SnippetsMode::Browse;
                }
            }
            SnippetsMode::Versions => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                    self.snippets.mode = SnippetsMode::Browse;
                }
            }
            SnippetsMode::SubmitPicker => self.snippets_submit_picker_key(key),
            SnippetsMode::SubmitFile => self.snippets_submit_file_key(key),
            SnippetsMode::SubmitFolder => self.snippets_submit_folder_key(key),
            SnippetsMode::SubmitStdin => self.snippets_submit_stdin_key(key),
            SnippetsMode::SubmitForm => self.snippets_submit_key(key),
            SnippetsMode::PatchForm => self.snippets_patch_key(key),
            SnippetsMode::FiltersForm => self.snippets_filters_key(key),
            SnippetsMode::SortPicker => self.snippets_sort_key(key),
            SnippetsMode::Confirm(kind) => self.snippets_confirm_key(key, kind),
            SnippetsMode::AddToList => self.snippets_add_to_list_key(key),
            SnippetsMode::Comments => self.snippets_comments_key(key),
        }
    }

    fn snippets_browse_key(&mut self, key: crossterm::event::KeyEvent) {
        if self.snippets.search_active {
            match key.code {
                KeyCode::Enter => {
                    self.snippets.search_active = false;
                    let mut si = blank_area("Search  (press / to focus)");
                    let content = self.snippets.search_input.lines().join(" ");
                    si.insert_str(&content);
                    self.snippets.search_input = si;
                    self.snippets.page = 1;
                    self.do_search();
                }
                KeyCode::Esc => {
                    self.snippets.search_active = false;
                }
                _ => {
                    self.snippets.search_input.input(key);
                }
            }
            return;
        }

        match key.code {
            KeyCode::Char('/') => {
                self.snippets.search_active = true;
                let content = self.snippets.search_input.lines().join(" ");
                let mut si = focused_area("Search  (Enter to run, Esc to cancel)");
                si.insert_str(&content);
                self.snippets.search_input = si;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = self.snippets.results.len();
                if len > 0 {
                    let i = self
                        .snippets
                        .table_state
                        .selected()
                        .map(|i| (i + 1) % len)
                        .unwrap_or(0);
                    self.snippets.table_state.select(Some(i));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let len = self.snippets.results.len();
                if len > 0 {
                    let i = self
                        .snippets
                        .table_state
                        .selected()
                        .map(|i| if i == 0 { len - 1 } else { i - 1 })
                        .unwrap_or(0);
                    self.snippets.table_state.select(Some(i));
                }
            }
            KeyCode::Enter => {
                if let Some(_id) = self.snippets.selected_id() {
                    self.snippets.mode = SnippetsMode::Actions;
                    self.snippets.action_state.select(Some(0));
                }
            }
            KeyCode::Char('n') => {
                self.snippets.submit_source_state.select(Some(0));
                self.snippets.mode = SnippetsMode::SubmitPicker;
            }
            KeyCode::Char('f') => {
                self.snippets.filters_form = SnippetFiltersForm::new_prefilled(
                    self.snippets.filter_tags.as_deref(),
                    self.snippets.filter_languages.as_deref(),
                    self.snippets.filter_submitter.as_deref(),
                    self.snippets.filter_generated.as_deref(),
                    self.snippets.filter_mine_only,
                );
                self.snippets.mode = SnippetsMode::FiltersForm;
            }
            KeyCode::Char('s') => {
                let idx = self
                    .snippets
                    .sort
                    .as_deref()
                    .and_then(|v| SNIPPET_SORT_OPTIONS.iter().position(|x| *x == v))
                    .unwrap_or(0);
                self.snippets.sort_state.select(Some(idx));
                self.snippets.mode = SnippetsMode::SortPicker;
            }
            KeyCode::Left => {
                let q = self.snippets.search_input.lines().join(" ");
                if self.snippets.page > 1 && !q.trim().is_empty() {
                    self.snippets.page -= 1;
                    self.do_search();
                }
            }
            KeyCode::Right => {
                let q = self.snippets.search_input.lines().join(" ");
                if !q.trim().is_empty() {
                    self.snippets.page = self.snippets.page.saturating_add(1);
                    self.do_search();
                }
            }
            KeyCode::Char('q') => self.should_quit = true,
            _ => {}
        }
    }

    fn snippets_detail_key(&mut self, key: crossterm::event::KeyEvent, scroll: u16) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.snippets.mode = SnippetsMode::Browse,
            KeyCode::Down | KeyCode::Char('j') => {
                self.snippets.mode = SnippetsMode::Detail { scroll: scroll + 1 };
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.snippets.mode = SnippetsMode::Detail {
                    scroll: scroll.saturating_sub(1),
                };
            }
            _ => {}
        }
    }

    fn snippets_actions_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.snippets.mode = SnippetsMode::Browse;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self
                    .snippets
                    .action_state
                    .selected()
                    .map(|i| (i + 1) % SNIPPET_ACTIONS.len())
                    .unwrap_or(0);
                self.snippets.action_state.select(Some(i));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self
                    .snippets
                    .action_state
                    .selected()
                    .map(|i| {
                        if i == 0 {
                            SNIPPET_ACTIONS.len() - 1
                        } else {
                            i - 1
                        }
                    })
                    .unwrap_or(0);
                self.snippets.action_state.select(Some(i));
            }
            KeyCode::Enter => {
                let action_idx = self.snippets.action_state.selected().unwrap_or(0);
                self.run_snippet_action(action_idx);
            }
            _ => {}
        }
    }

    fn run_snippet_action(&mut self, idx: usize) {
        let id = match self.snippets.selected_id() {
            Some(id) => id,
            None => return,
        };
        match idx {
            0 => {
                self.load_snippet_detail(id);
            }
            1 => {
                self.do_vote(id, "up");
                self.snippets.mode = SnippetsMode::Browse;
            }
            2 => {
                self.do_vote(id, "down");
                self.snippets.mode = SnippetsMode::Browse;
            }
            3 => {
                self.do_vote(id, "remove");
                self.snippets.mode = SnippetsMode::Browse;
            }
            4 => {
                self.do_bookmark(id, true);
                self.snippets.mode = SnippetsMode::Browse;
            }
            5 => {
                self.do_bookmark(id, false);
                self.snippets.mode = SnippetsMode::Browse;
            }
            6 => {
                self.load_snippet_metrics(id);
            }
            7 => {
                self.load_snippet_versions(id);
            }
            8 => {
                self.snippets.add_to_list_input =
                    blank_area("List ID (Enter to confirm, Esc to cancel)");
                self.snippets.mode = SnippetsMode::AddToList;
            }
            9 => {
                if let Some(detail) = &self.snippets.detail {
                    let d = detail.clone();
                    let tags = d
                        .get("tags")
                        .and_then(|t| t.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_default();
                    self.snippets.patch_form = SnippetForm::new_prefilled(
                        &sv(&d, "title"),
                        &sv(&d, "description"),
                        &sv(&d, "language"),
                        &tags,
                    );
                } else {
                    self.snippets.patch_form = SnippetForm::new();
                }
                self.snippets.mode = SnippetsMode::PatchForm;
            }
            10 => {
                self.snippets.mode = SnippetsMode::Confirm(ConfirmKind::DeleteSnippet(id));
            }
            _ => {}
        }
    }

    fn snippets_submit_picker_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.snippets.mode = SnippetsMode::Browse;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = SNIPPET_SUBMIT_SOURCES.len();
                let i = self
                    .snippets
                    .submit_source_state
                    .selected()
                    .map(|i| (i + 1) % len)
                    .unwrap_or(0);
                self.snippets.submit_source_state.select(Some(i));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let len = SNIPPET_SUBMIT_SOURCES.len();
                let i = self
                    .snippets
                    .submit_source_state
                    .selected()
                    .map(|i| if i == 0 { len - 1 } else { i - 1 })
                    .unwrap_or(0);
                self.snippets.submit_source_state.select(Some(i));
            }
            KeyCode::Enter => match self.snippets.submit_source_state.selected().unwrap_or(0) {
                0 => {
                    self.snippets.submit_file_input =
                        focused_area("File path (Enter submit, Esc cancel)");
                    self.snippets.mode = SnippetsMode::SubmitFile;
                }
                1 => {
                    self.snippets.submit_folder_input =
                        focused_area("Folder path (Enter submit, Esc cancel)");
                    self.snippets.mode = SnippetsMode::SubmitFolder;
                }
                2 => {
                    self.snippets.submit_stdin_input =
                        focused_area("Snippet JSON payload (F10/F5 submit, Esc cancel)");
                    self.snippets.mode = SnippetsMode::SubmitStdin;
                }
                _ => {
                    self.snippets.submit_form = SnippetForm::new();
                    self.snippets.mode = SnippetsMode::SubmitForm;
                }
            },
            _ => {}
        }
    }

    fn snippets_submit_file_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => self.snippets.mode = SnippetsMode::SubmitPicker,
            KeyCode::Enter => self.do_submit_file(),
            _ => {
                self.snippets.submit_file_input.input(key);
            }
        }
    }

    fn snippets_submit_folder_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => self.snippets.mode = SnippetsMode::SubmitPicker,
            KeyCode::Enter => self.do_submit_folder(),
            _ => {
                self.snippets.submit_folder_input.input(key);
            }
        }
    }

    fn snippets_submit_stdin_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => self.snippets.mode = SnippetsMode::SubmitPicker,
            KeyCode::F(10) | KeyCode::F(5) => self.do_submit_stdin_json(),
            _ => {
                self.snippets.submit_stdin_input.input(key);
            }
        }
    }

    fn snippets_submit_key(&mut self, key: crossterm::event::KeyEvent) {
        let focus = self.snippets.submit_form.focus;
        match key.code {
            KeyCode::Esc => {
                self.snippets.mode = SnippetsMode::SubmitPicker;
            }
            KeyCode::Tab => {
                self.snippets.submit_form.next_field();
            }
            KeyCode::BackTab => {
                self.snippets.submit_form.prev_field();
            }
            KeyCode::F(10) | KeyCode::F(5) => {
                self.do_submit_snippet();
            }
            KeyCode::Enter if focus != 4 => {
                self.snippets.submit_form.next_field();
            }
            _ => {
                self.snippets.submit_form.fields[focus].input(key);
            }
        }
    }

    fn snippets_patch_key(&mut self, key: crossterm::event::KeyEvent) {
        let id = match self.snippets.selected_id() {
            Some(id) => id,
            None => {
                self.snippets.mode = SnippetsMode::Browse;
                return;
            }
        };
        let focus = self.snippets.patch_form.focus;
        match key.code {
            KeyCode::Esc => {
                self.snippets.mode = SnippetsMode::Browse;
            }
            KeyCode::Tab => {
                self.snippets.patch_form.next_field();
            }
            KeyCode::BackTab => {
                self.snippets.patch_form.prev_field();
            }
            KeyCode::F(10) => {
                self.do_patch_snippet(id);
            }
            _ => {
                self.snippets.patch_form.fields[focus].input(key);
            }
        }
    }

    fn snippets_filters_key(&mut self, key: crossterm::event::KeyEvent) {
        let focus = self.snippets.filters_form.focus;
        match key.code {
            KeyCode::Esc => {
                self.snippets.mode = SnippetsMode::Browse;
            }
            KeyCode::Tab => {
                self.snippets.filters_form.next_field();
            }
            KeyCode::BackTab => {
                self.snippets.filters_form.prev_field();
            }
            KeyCode::Enter => {
                if focus == self.snippets.filters_form.fields.len() - 1 {
                    self.apply_filters();
                } else {
                    self.snippets.filters_form.next_field();
                }
            }
            KeyCode::F(10) | KeyCode::F(5) => {
                self.apply_filters();
            }
            _ => {
                self.snippets.filters_form.fields[focus].input(key);
            }
        }
    }

    fn apply_filters(&mut self) {
        let to_opt = |s: String| {
            let t = s.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        };
        let tags = to_opt(self.snippets.filters_form.value(0));
        let languages = to_opt(self.snippets.filters_form.value(1));
        let submitter = to_opt(self.snippets.filters_form.value(2));
        let generated = to_opt(self.snippets.filters_form.value(3));
        let mine_raw = self.snippets.filters_form.value(4).to_lowercase();
        let mine_only = if mine_raw.trim().is_empty()
            || matches!(mine_raw.trim(), "no" | "n" | "false" | "0" | "off")
        {
            false
        } else if matches!(mine_raw.trim(), "yes" | "y" | "true" | "1" | "on") {
            true
        } else {
            self.snippets.error =
                Some("My snippets only must be yes/no (or true/false).".to_string());
            return;
        };

        if let Some(g) = generated.as_deref() {
            if !matches!(g, "include" | "exclude" | "only") {
                self.snippets.error = Some(
                    "AI Generated must be one of: include, exclude, only.".to_string(),
                );
                return;
            }
        }

        self.snippets.filter_tags = tags;
        self.snippets.filter_languages = languages;
        self.snippets.filter_submitter = submitter;
        self.snippets.filter_generated = generated;
        self.snippets.filter_mine_only = mine_only;
        self.snippets.page = 1;
        self.snippets.mode = SnippetsMode::Browse;
        self.do_search();
    }

    fn snippets_sort_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.snippets.mode = SnippetsMode::Browse;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = SNIPPET_SORT_OPTIONS.len();
                let i = self
                    .snippets
                    .sort_state
                    .selected()
                    .map(|i| (i + 1) % len)
                    .unwrap_or(0);
                self.snippets.sort_state.select(Some(i));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let len = SNIPPET_SORT_OPTIONS.len();
                let i = self
                    .snippets
                    .sort_state
                    .selected()
                    .map(|i| if i == 0 { len - 1 } else { i - 1 })
                    .unwrap_or(0);
                self.snippets.sort_state.select(Some(i));
            }
            KeyCode::Enter | KeyCode::F(10) | KeyCode::F(5) => {
                let idx = self.snippets.sort_state.selected().unwrap_or(0);
                self.snippets.sort = Some(SNIPPET_SORT_OPTIONS[idx].to_string());
                self.snippets.page = 1;
                self.snippets.mode = SnippetsMode::Browse;
                self.do_search();
            }
            _ => {}
        }
    }

    fn snippets_confirm_key(&mut self, key: crossterm::event::KeyEvent, kind: ConfirmKind) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => match kind {
                ConfirmKind::DeleteSnippet(id) => {
                    self.do_delete_snippet(id);
                    self.snippets.mode = SnippetsMode::Browse;
                }
                _ => {
                    self.snippets.mode = SnippetsMode::Browse;
                }
            },
            _ => {
                self.snippets.mode = SnippetsMode::Browse;
            }
        }
    }

    fn snippets_add_to_list_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.snippets.mode = SnippetsMode::Browse;
            }
            KeyCode::Enter => {
                let list_id = self
                    .snippets
                    .add_to_list_input
                    .lines()
                    .first()
                    .cloned()
                    .unwrap_or_default();
                if !list_id.trim().is_empty() {
                    self.do_add_to_list(list_id.trim().to_string());
                }
                self.snippets.mode = SnippetsMode::Browse;
            }
            _ => {
                self.snippets.add_to_list_input.input(key);
            }
        }
    }

    fn snippets_comments_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.snippets.mode = SnippetsMode::Browse;
            }
            _ => {}
        }
    }

    fn handle_lists_key(&mut self, key: crossterm::event::KeyEvent) {
        self.lists.success = None;
        let mode = self.lists.mode.clone();
        match mode {
            ListsMode::Browse => self.lists_browse_key(key),
            ListsMode::Detail => self.lists_detail_key(key),
            ListsMode::CreateForm => self.lists_create_key(key),
            ListsMode::EditForm => self.lists_edit_key(key),
            ListsMode::AddSnippet => self.lists_add_snippet_key(key),
            ListsMode::Confirm(kind) => self.lists_confirm_key(key, kind),
        }
    }

    fn lists_browse_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                let len = self.lists.my_lists.len();
                if len > 0 {
                    let i = self
                        .lists
                        .table_state
                        .selected()
                        .map(|i| (i + 1) % len)
                        .unwrap_or(0);
                    self.lists.table_state.select(Some(i));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let len = self.lists.my_lists.len();
                if len > 0 {
                    let i = self
                        .lists
                        .table_state
                        .selected()
                        .map(|i| if i == 0 { len - 1 } else { i - 1 })
                        .unwrap_or(0);
                    self.lists.table_state.select(Some(i));
                }
            }
            KeyCode::Enter => {
                if let Some(id) = self.lists.selected_id() {
                    self.load_list_detail(id);
                }
            }
            KeyCode::Char('n') => {
                self.lists.create_form = ListForm::new();
                self.lists.mode = ListsMode::CreateForm;
            }
            KeyCode::Char('d') => {
                if let Some(id) = self.lists.selected_id() {
                    self.lists.mode = ListsMode::Confirm(ConfirmKind::DeleteList(id));
                }
            }
            KeyCode::Char('r') => {
                self.load_my_lists();
            }
            KeyCode::Char('q') => self.should_quit = true,
            _ => {}
        }
    }

    fn lists_detail_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.lists.mode = ListsMode::Browse;
            }
            KeyCode::Char('a') => {
                self.lists.add_snippet_input =
                    blank_area("Snippet ID (Enter to add, Esc to cancel)");
                self.lists.mode = ListsMode::AddSnippet;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(d) = &self.lists.detail {
                    let len = d
                        .get("snippets")
                        .and_then(|s| s.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    if len > 0 {
                        let i = self
                            .lists
                            .detail_table
                            .selected()
                            .map(|i| (i + 1) % len)
                            .unwrap_or(0);
                        self.lists.detail_table.select(Some(i));
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(d) = &self.lists.detail {
                    let len = d
                        .get("snippets")
                        .and_then(|s| s.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    if len > 0 {
                        let i = self
                            .lists
                            .detail_table
                            .selected()
                            .map(|i| if i == 0 { len - 1 } else { i - 1 })
                            .unwrap_or(0);
                        self.lists.detail_table.select(Some(i));
                    }
                }
            }
            _ => {}
        }
    }

    fn lists_create_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.lists.mode = ListsMode::Browse;
            }
            KeyCode::Tab => {
                self.lists.create_form.next_field();
            }
            KeyCode::F(10) => {
                self.do_create_list();
            }
            KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                self.lists.create_form.unlisted = !self.lists.create_form.unlisted;
            }
            _ => {
                let focus = self.lists.create_form.focus;
                self.lists.create_form.fields[focus].input(key);
            }
        }
    }

    fn lists_edit_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.lists.mode = ListsMode::Browse;
            }
            _ => {}
        }
    }

    fn lists_add_snippet_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.lists.mode = ListsMode::Detail;
            }
            KeyCode::Enter => {
                self.do_add_snippet_to_list();
                self.lists.mode = ListsMode::Detail;
            }
            _ => {
                self.lists.add_snippet_input.input(key);
            }
        }
    }

    fn lists_confirm_key(&mut self, key: crossterm::event::KeyEvent, kind: ConfirmKind) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let ConfirmKind::DeleteList(id) = kind {
                    self.do_delete_list(id);
                }
                self.lists.mode = ListsMode::Browse;
            }
            _ => {
                self.lists.mode = ListsMode::Browse;
            }
        }
    }

    fn handle_requests_key(&mut self, key: crossterm::event::KeyEvent) {
        self.requests.success = None;
        let mode = self.requests.mode.clone();
        match mode {
            RequestsMode::Browse => self.requests_browse_key(key),
            RequestsMode::Detail => self.requests_detail_key(key),
            RequestsMode::SubmitForm => self.requests_submit_key(key),
            RequestsMode::Confirm(kind) => self.requests_confirm_key(key, kind),
        }
    }

    fn requests_browse_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                let len = self.requests.requests.len();
                if len > 0 {
                    let i = self
                        .requests
                        .table_state
                        .selected()
                        .map(|i| (i + 1) % len)
                        .unwrap_or(0);
                    self.requests.table_state.select(Some(i));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let len = self.requests.requests.len();
                if len > 0 {
                    let i = self
                        .requests
                        .table_state
                        .selected()
                        .map(|i| if i == 0 { len - 1 } else { i - 1 })
                        .unwrap_or(0);
                    self.requests.table_state.select(Some(i));
                }
            }
            KeyCode::Enter => {
                if let Some(id) = self.requests.selected_id() {
                    self.load_request_detail(id);
                }
            }
            KeyCode::Char('n') => {
                self.requests.submit_form = RequestForm::new();
                self.requests.mode = RequestsMode::SubmitForm;
            }
            KeyCode::Char('d') => {
                if let Some(id) = self.requests.selected_id() {
                    self.requests.mode = RequestsMode::Confirm(ConfirmKind::DeleteRequest(id));
                }
            }
            KeyCode::Char('r') => {
                self.load_requests();
            }
            KeyCode::Char('q') => self.should_quit = true,
            _ => {}
        }
    }

    fn requests_detail_key(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.requests.mode = RequestsMode::Browse;
            }
            _ => {}
        }
    }

    fn requests_submit_key(&mut self, key: crossterm::event::KeyEvent) {
        let focus = self.requests.submit_form.focus;
        match key.code {
            KeyCode::Esc => {
                self.requests.mode = RequestsMode::Browse;
            }
            KeyCode::Tab => {
                self.requests.submit_form.next_field();
            }
            KeyCode::BackTab => {
                let f = &mut self.requests.submit_form;
                if f.focus == 0 {
                    f.focus = f.fields.len() - 1;
                } else {
                    f.focus -= 1;
                }
                f.refresh_styles();
            }
            KeyCode::F(10) => {
                self.do_submit_request();
            }
            KeyCode::Enter if focus != 1 => {
                self.requests.submit_form.next_field();
            }
            _ => {
                self.requests.submit_form.fields[focus].input(key);
            }
        }
    }

    fn requests_confirm_key(&mut self, key: crossterm::event::KeyEvent, kind: ConfirmKind) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let ConfirmKind::DeleteRequest(id) = kind {
                    self.do_delete_request(id);
                }
                self.requests.mode = RequestsMode::Browse;
            }
            _ => {
                self.requests.mode = RequestsMode::Browse;
            }
        }
    }

    fn handle_commands_key(&mut self, key: crossterm::event::KeyEvent) {
        self.commands.success = None;
        if self.commands.input_active {
            match key.code {
                KeyCode::Enter => {
                    let cmd = self.commands.command_text();
                    self.run_cli_command(cmd);
                }
                KeyCode::Esc => self.commands.set_input_active(false),
                KeyCode::Up if key.modifiers == KeyModifiers::CONTROL => {
                    if self.commands.history.is_empty() {
                        return;
                    }
                    let i = self
                        .commands
                        .history_cursor
                        .map(|n| n.saturating_sub(1))
                        .unwrap_or(self.commands.history.len() - 1);
                    self.commands.history_cursor = Some(i);
                    if let Some(cmd) = self.commands.history.get(i).cloned() {
                        self.commands.set_input_text(&cmd);
                    }
                }
                KeyCode::Down if key.modifiers == KeyModifiers::CONTROL => {
                    if self.commands.history.is_empty() {
                        return;
                    }
                    let i = self
                        .commands
                        .history_cursor
                        .map(|n| (n + 1) % self.commands.history.len())
                        .unwrap_or(0);
                    self.commands.history_cursor = Some(i);
                    if let Some(cmd) = self.commands.history.get(i).cloned() {
                        self.commands.set_input_text(&cmd);
                    }
                }
                _ => {
                    self.commands.input.input(key);
                }
            }
            return;
        }

        match key.code {
            KeyCode::Char('/') | KeyCode::Char('i') => self.commands.set_input_active(true),
            KeyCode::Char('c') => {
                self.commands.output.clear();
                self.commands.error = None;
                self.commands.success = None;
            }
            KeyCode::Char('r') => {
                if let Some(cmd) = self.commands.last_command.clone() {
                    self.run_cli_command(cmd);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.commands.scroll = self.commands.scroll.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.commands.scroll = self.commands.scroll.saturating_sub(1);
            }
            KeyCode::Char('q') => self.should_quit = true,
            _ => {}
        }
    }
}

fn render(app: &mut App, f: &mut Frame) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    render_tab_bar(app, f, chunks[0]);

    match app.tab {
        Tab::Overview => render_overview(app, f, chunks[1]),
        Tab::Snippets => render_snippets(app, f, chunks[1]),
        Tab::Lists => render_lists(app, f, chunks[1]),
        Tab::Requests => render_requests(app, f, chunks[1]),
        Tab::Commands => render_commands(app, f, chunks[1]),
    }

    render_status_bar(app, f, chunks[2]);

    if app.show_help {
        render_help(f, area);
    }
}

fn render_tab_bar(app: &App, f: &mut Frame, area: Rect) {
    let titles: Vec<Line> = TABS
        .iter()
        .map(|t| {
            Line::from(Span::styled(
                format!("  {}  ", t.label()),
                if *t == app.tab { bold_teal() } else { dim() },
            ))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(teal())
                .title(Span::styled(" microcodes ", bold_teal())),
        )
        .select(TABS.iter().position(|t| *t == app.tab).unwrap_or(0))
        .highlight_style(selected())
        .divider(Span::styled("|", dim()));

    f.render_widget(tabs, area);
}

fn render_status_bar(app: &App, f: &mut Frame, area: Rect) {
    let hints = match app.tab {
        Tab::Overview => " r:Refresh  1-5:Switch tab  q:Quit  ?:Help",
        Tab::Snippets => match &app.snippets.mode {
            SnippetsMode::Browse => {
                " /:Search  f:Filters  s:Sort  ←/→:Page  ↑↓/jk:Navigate  Enter:Actions"
            }
            SnippetsMode::Actions => " ↑↓/jk:Navigate  Enter:Select  Esc:Back",
            SnippetsMode::Detail { .. } => " ↑↓/jk:Scroll  Esc:Back",
            SnippetsMode::SubmitPicker => " ↑↓/jk:Select source  Enter:Choose  Esc:Cancel",
            SnippetsMode::SubmitFile => " Enter:Submit file  Esc:Back  (path input)",
            SnippetsMode::SubmitFolder => " Enter:Submit folder  Esc:Back  (path input)",
            SnippetsMode::SubmitStdin => " F10/F5:Submit JSON  Esc:Back",
            SnippetsMode::SubmitForm => " Tab:Next field  Enter:Next (code field allows newlines)  F10:Submit  Esc:Cancel",
            SnippetsMode::PatchForm => " Tab:Next field  F10:Patch  Esc:Cancel",
            SnippetsMode::FiltersForm => " Tab:Next field  Enter:Next/Apply  F10:Apply  Esc:Cancel",
            SnippetsMode::SortPicker => " ↑↓/jk:Pick sort  Enter:Apply  Esc:Cancel",
            SnippetsMode::Confirm(_) => " y:Confirm  any other:Cancel",
            SnippetsMode::Metrics => " Esc:Back",
            SnippetsMode::Versions => " Esc:Back",
            SnippetsMode::AddToList => " Enter:Add  Esc:Cancel",
            SnippetsMode::Comments => " Esc:Back",
        },
        Tab::Lists => match &app.lists.mode {
            ListsMode::Browse => {
                " ↑↓/jk:Navigate  Enter:View  n:New  d:Delete  r:Refresh  q:Quit  ?:Help"
            }
            ListsMode::Detail => " ↑↓/jk:Navigate  a:Add snippet  Esc:Back",
            ListsMode::CreateForm => {
                " Tab:Next field  Ctrl+u:Toggle unlisted  F10:Create  Esc:Cancel"
            }
            ListsMode::AddSnippet => " Enter:Add  Esc:Cancel",
            ListsMode::Confirm(_) => " y:Confirm  any other:Cancel",
            _ => " Esc:Back",
        },
        Tab::Requests => match &app.requests.mode {
            RequestsMode::Browse => {
                " ↑↓/jk:Navigate  Enter:View  n:New  d:Delete  r:Refresh  q:Quit  ?:Help"
            }
            RequestsMode::Detail => " Esc:Back",
            RequestsMode::SubmitForm => " Tab:Next field  Enter:Next  F10:Submit  Esc:Cancel",
            RequestsMode::Confirm(_) => " y:Confirm  any other:Cancel",
        },
        Tab::Commands => {
            if app.commands.input_active {
                " Enter:Run  Ctrl+↑/↓:History  Esc:Unfocus  1-5:Tabs  ?:Help"
            } else {
                " /:Focus input  r:Rerun  c:Clear output  ↑↓/jk:Scroll  q:Quit  ?:Help"
            }
        }
    };

    let loading_indicator = if app.snippets.loading
        || app.lists.loading
        || app.requests.loading
        || app.overview.loading
        || app.commands.loading
    {
        Span::styled(" ⣾ Loading…", Style::default().fg(YELLOW))
    } else {
        Span::raw("")
    };

    let line = Line::from(vec![Span::styled(hints, dim()), loading_indicator]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_overview(app: &mut App, f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(teal())
        .title(Span::styled(" Overview ", bold_teal()));

    if app.overview.loading {
        let p = Paragraph::new("Loading…").style(dim()).block(block);
        f.render_widget(p, area);
        return;
    }

    if let Some(e) = &app.overview.error.clone() {
        let p = Paragraph::new(format!("Error: {}", e))
            .style(Style::default().fg(RED))
            .block(block);
        f.render_widget(p, area);
        return;
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(profile) = &app.overview.profile.clone() {
        let p = profile.clone();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(10), Constraint::Min(0)])
            .split(inner);

        let role = sv(&p, "role");
        let role_color = match role.as_str() {
            "admin" => Color::Red,
            "moderator" => YELLOW,
            _ => TEAL,
        };

        let lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("  Username:  ", header_style()),
                Span::styled(
                    sv(&p, "username"),
                    Style::default().fg(FG).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  ID:        ", header_style()),
                Span::raw(sv(&p, "id")),
            ]),
            Line::from(vec![
                Span::styled("  Email:     ", header_style()),
                Span::raw(sv(&p, "email")),
            ]),
            Line::from(vec![
                Span::styled("  Bio:       ", header_style()),
                Span::raw(sv(&p, "description")),
            ]),
            Line::from(vec![
                Span::styled("  Role:      ", header_style()),
                Span::styled(
                    role,
                    Style::default().fg(role_color).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Created:   ", header_style()),
                Span::raw(sv(&p, "createdAt")),
            ]),
        ];

        let profile_para = Paragraph::new(lines).wrap(Wrap { trim: false });
        f.render_widget(profile_para, chunks[0]);

        let hint = Paragraph::new(Span::styled(
            "\n  Press r to refresh   Press 2-5 to navigate to Snippets / Lists / Requests / Commands",
            dim(),
        ));
        f.render_widget(hint, chunks[1]);
    } else if app.api.token.is_none() {
        let p = Paragraph::new(vec![
            Line::from(Span::styled(
                "  Not authenticated.",
                Style::default().fg(YELLOW),
            )),
            Line::from(""),
            Line::from(Span::styled("  Set MICROCODES_API_TOKEN to log in.", dim())),
            Line::from(""),
            Line::from(Span::styled(
                "  You can still browse Snippets, Lists, and Requests.",
                dim(),
            )),
        ])
        .block(Block::default());
        f.render_widget(p, inner);
    } else {
        let p = Paragraph::new("Press r to load profile.").style(dim());
        f.render_widget(p, inner);
    }
}

fn render_snippets(app: &mut App, f: &mut Frame, area: Rect) {
    match app.snippets.mode.clone() {
        SnippetsMode::SubmitPicker => {
            render_snippet_submit_picker(app, f, area);
            return;
        }
        SnippetsMode::SubmitFile => {
            render_snippet_submit_file(app, f, area);
            return;
        }
        SnippetsMode::SubmitFolder => {
            render_snippet_submit_folder(app, f, area);
            return;
        }
        SnippetsMode::SubmitStdin => {
            render_snippet_submit_stdin(app, f, area);
            return;
        }
        SnippetsMode::SubmitForm => {
            render_snippet_form(app, f, area, false);
            return;
        }
        SnippetsMode::PatchForm => {
            render_snippet_form(app, f, area, true);
            return;
        }
        SnippetsMode::Detail { scroll } => {
            render_snippet_detail_full(app, f, area, scroll);
            return;
        }
        SnippetsMode::Metrics => {
            render_snippet_metrics(app, f, area);
            return;
        }
        SnippetsMode::Versions => {
            render_snippet_versions(app, f, area);
            return;
        }
        _ => {}
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    f.render_widget(&app.snippets.search_input, chunks[0]);

    let table_area = chunks[1];

    if let Some(e) = &app.snippets.error.clone() {
        let popup = centered_rect(60, 20, table_area);
        f.render_widget(Clear, popup);
        let p = Paragraph::new(format!("  Error: {}", e))
            .style(Style::default().fg(RED))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(RED))
                    .title(Span::styled(
                        " Error ",
                        Style::default().fg(RED).add_modifier(Modifier::BOLD),
                    )),
            );
        f.render_widget(p, popup);
        return;
    }

    if let Some(s) = &app.snippets.success.clone() {
        let popup = centered_rect(50, 10, table_area);
        f.render_widget(Clear, popup);
        let p = Paragraph::new(format!("  {}", s))
            .style(Style::default().fg(GREEN))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(GREEN))
                    .title(Span::styled(
                        " Success ",
                        Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
                    )),
            );
        f.render_widget(p, popup);
        return;
    }

    if app.snippets.loading {
        let p = Paragraph::new("  Loading…")
            .style(dim())
            .block(Block::default().borders(Borders::ALL).border_style(teal()));
        f.render_widget(p, table_area);
    } else {
        render_snippets_table(app, f, table_area);
    }

    if app.snippets.mode == SnippetsMode::Actions {
        render_actions_popup(app, f, area);
    }
    if app.snippets.mode == SnippetsMode::FiltersForm {
        render_snippet_filters_popup(app, f, area);
    }
    if app.snippets.mode == SnippetsMode::SortPicker {
        render_snippet_sort_popup(app, f, area);
    }
    if app.snippets.mode == SnippetsMode::AddToList {
        render_add_to_list_popup(app, f, area);
    }
    if let SnippetsMode::Confirm(ref kind) = app.snippets.mode.clone() {
        render_confirm_popup(f, area, confirm_message(kind));
    }
}

fn render_snippets_table(app: &mut App, f: &mut Frame, area: Rect) {
    let mut title_parts = vec![
        format!("page {}", app.snippets.page),
        format!(
            "sort {}",
            app.snippets.sort.as_deref().unwrap_or("relevance")
        ),
    ];
    if let Some(v) = app.snippets.filter_tags.as_deref() {
        if !v.is_empty() {
            title_parts.push(format!("tags={}", truncate(v, 20)));
        }
    }
    if let Some(v) = app.snippets.filter_languages.as_deref() {
        if !v.is_empty() {
            title_parts.push(format!("lang={}", truncate(v, 20)));
        }
    }
    if app.snippets.filter_mine_only {
        title_parts.push("mine=yes".to_string());
    } else if let Some(v) = app.snippets.filter_submitter.as_deref() {
        if !v.is_empty() {
            title_parts.push(format!("by={}", truncate(v, 16)));
        }
    }
    if let Some(v) = app.snippets.filter_generated.as_deref() {
        if !v.is_empty() {
            title_parts.push(format!("ai={}", v));
        }
    }
    let title = format!(" Snippets ({}) ", title_parts.join(" | "));

    let header = Row::new(vec![
        Cell::from("ID").style(header_style()),
        Cell::from("Title").style(header_style()),
        Cell::from("Language").style(header_style()),
        Cell::from("Upvotes").style(header_style()),
    ])
    .height(1)
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .snippets
        .results
        .iter()
        .map(|s| {
            Row::new(vec![
                Cell::from(truncate(&sv(s, "id"), 24)),
                Cell::from(truncate(&sv(s, "title"), 42)),
                Cell::from(sv(s, "language")),
                Cell::from(nv(s, "upvotes")),
            ])
        })
        .collect();

    let hint = if app.snippets.results.is_empty() {
        "  Press / to search. Use id:<id1,id2> for direct fetch; f filters, s sort, ←/→ pages."
    } else {
        ""
    };

    if rows.is_empty() {
        let p = Paragraph::new(hint).style(dim()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(teal())
                .title(Span::styled(title.clone(), bold_teal())),
        );
        f.render_widget(p, area);
        return;
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(26),
            Constraint::Min(20),
            Constraint::Length(14),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(teal())
            .title(Span::styled(title, bold_teal())),
    )
    .row_highlight_style(selected())
    .highlight_symbol("▶ ");

    f.render_stateful_widget(table, area, &mut app.snippets.table_state);
}

fn render_actions_popup(app: &mut App, f: &mut Frame, area: Rect) {
    let popup = centered_rect(40, 70, area);
    f.render_widget(Clear, popup);

    let items: Vec<ListItem> = SNIPPET_ACTIONS
        .iter()
        .map(|a| ListItem::new(format!("  {}  ", a)))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(bold_teal())
                .title(Span::styled(" Actions ", bold_teal())),
        )
        .highlight_style(selected())
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, popup, &mut app.snippets.action_state);
}

fn render_snippet_submit_picker(app: &mut App, f: &mut Frame, area: Rect) {
    let popup = centered_rect(42, 52, area);
    f.render_widget(Clear, popup);

    let items: Vec<ListItem> = SNIPPET_SUBMIT_SOURCES
        .iter()
        .map(|s| ListItem::new(format!("  {}  ", s)))
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(bold_teal())
                .title(Span::styled(
                    " New Snippet Source (↑↓/jk, Enter select, Esc cancel) ",
                    bold_teal(),
                )),
        )
        .highlight_style(selected())
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, popup, &mut app.snippets.submit_source_state);
}

fn render_snippet_submit_file(app: &mut App, f: &mut Frame, area: Rect) {
    let popup = centered_rect(70, 24, area);
    f.render_widget(Clear, popup);
    f.render_widget(&app.snippets.submit_file_input, popup);
}

fn render_snippet_submit_folder(app: &mut App, f: &mut Frame, area: Rect) {
    let popup = centered_rect(70, 24, area);
    f.render_widget(Clear, popup);
    f.render_widget(&app.snippets.submit_folder_input, popup);
}

fn render_snippet_submit_stdin(app: &mut App, f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(teal())
        .title(Span::styled(
            " New Snippet JSON via Stdin (F10/F5 submit | Esc back) ",
            bold_teal(),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(&app.snippets.submit_stdin_input, inner);
}

fn render_snippet_filters_popup(app: &mut App, f: &mut Frame, area: Rect) {
    let content_height = (5 * 3 + 5).min(area.height);
    let popup_width = (area.width as f32 * 0.72) as u16;
    let popup = Rect {
        x: (area.width - popup_width) / 2,
        y: (area.height - content_height) / 2,
        width: popup_width,
        height: content_height,
    };
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(bold_teal())
        .title(Span::styled(
            " Snippet Filters — Tab/Shift+Tab: move  F10: apply  Esc: cancel ",
            bold_teal(),
        ));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Min(0),
        ])
        .split(inner);

    f.render_widget(&app.snippets.filters_form.fields[0], chunks[0]);
    f.render_widget(&app.snippets.filters_form.fields[1], chunks[1]);
    f.render_widget(&app.snippets.filters_form.fields[2], chunks[2]);
    f.render_widget(&app.snippets.filters_form.fields[3], chunks[3]);
    f.render_widget(&app.snippets.filters_form.fields[4], chunks[4]);

    let help = Paragraph::new(vec![
        Line::from(Span::styled(
            "  Generated: include | exclude | only     Mine-only: yes | no",
            dim(),
        )),
        Line::from(Span::styled(
            "  Leave fields empty to remove that filter.  Enter on last field applies.",
            dim(),
        )),
    ]);
    f.render_widget(help, chunks[5]);
}

fn render_snippet_sort_popup(app: &mut App, f: &mut Frame, area: Rect) {
    let popup = centered_rect(36, 40, area);
    f.render_widget(Clear, popup);

    let items: Vec<ListItem> = SNIPPET_SORT_OPTIONS
        .iter()
        .map(|v| ListItem::new(format!("  {}", v)))
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(bold_teal())
                .title(Span::styled(
                    " Sort (↑↓/jk, Enter apply, Esc cancel) ",
                    bold_teal(),
                )),
        )
        .highlight_style(selected())
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, popup, &mut app.snippets.sort_state);
}

fn render_add_to_list_popup(app: &mut App, f: &mut Frame, area: Rect) {
    let popup = centered_rect(40, 20, area);
    f.render_widget(Clear, popup);
    f.render_widget(&app.snippets.add_to_list_input, popup);
}

fn render_snippet_detail_full(app: &mut App, f: &mut Frame, area: Rect, scroll: u16) {
    let detail = match &app.snippets.detail {
        Some(d) => d.clone(),
        None => {
            return;
        }
    };

    let tags = detail
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let code = sv(&detail, "code");

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("  Title:        ", header_style()),
            Span::raw(sv(&detail, "title")),
        ]),
        Line::from(vec![
            Span::styled("  ID:           ", header_style()),
            Span::raw(sv(&detail, "id")),
        ]),
        Line::from(vec![
            Span::styled("  Language:     ", header_style()),
            Span::raw(sv(&detail, "language")),
        ]),
        Line::from(vec![
            Span::styled("  Description:  ", header_style()),
            Span::raw(sv(&detail, "description")),
        ]),
        Line::from(vec![
            Span::styled("  Tags:         ", header_style()),
            Span::raw(tags),
        ]),
        Line::from(vec![
            Span::styled("  Submitter:    ", header_style()),
            Span::raw(sv(&detail, "submitter")),
        ]),
        Line::from(vec![
            Span::styled("  Upvotes:      ", header_style()),
            Span::raw(nv(&detail, "upvotes")),
        ]),
        Line::from(vec![
            Span::styled("  Downvotes:    ", header_style()),
            Span::raw(nv(&detail, "downvotes")),
        ]),
        Line::from(vec![
            Span::styled("  Bookmarks:    ", header_style()),
            Span::raw(nv(&detail, "bookmarks")),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Code:", bold_teal())),
        Line::from(""),
    ];

    for l in code.lines() {
        lines.push(Line::from(format!("    {}", l)));
    }

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(teal())
                .title(Span::styled(
                    " Snippet Detail  (Esc to go back) ",
                    bold_teal(),
                )),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(p, area);
}

fn render_snippet_metrics(app: &mut App, f: &mut Frame, area: Rect) {
    let metrics = match &app.snippets.metrics {
        Some(m) => m.clone(),
        None => return,
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Upvotes:    ", header_style()),
            Span::raw(nv(&metrics, "upvotes")),
        ]),
        Line::from(vec![
            Span::styled("  Downvotes:  ", header_style()),
            Span::raw(nv(&metrics, "downvotes")),
        ]),
        Line::from(vec![
            Span::styled("  Bookmarks:  ", header_style()),
            Span::raw(nv(&metrics, "bookmarks")),
        ]),
    ];

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(teal())
            .title(Span::styled(" Metrics  (Esc to go back) ", bold_teal())),
    );
    f.render_widget(p, area);
}

fn render_snippet_versions(app: &mut App, f: &mut Frame, area: Rect) {
    let header = Row::new(vec![
        Cell::from("Version").style(header_style()),
        Cell::from("Modified").style(header_style()),
        Cell::from("Editor").style(header_style()),
    ])
    .height(1)
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .snippets
        .versions
        .iter()
        .map(|v| {
            Row::new(vec![
                Cell::from(nv(v, "version")),
                Cell::from(sv(v, "modifiedAt")),
                Cell::from(sv(v, "editor")),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Min(20),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(teal())
            .title(Span::styled(" Versions  (Esc to go back) ", bold_teal())),
    )
    .row_highlight_style(selected());

    f.render_stateful_widget(table, area, &mut app.snippets.versions_state);
}

fn render_snippet_form(app: &mut App, f: &mut Frame, area: Rect, is_patch: bool) {
    let title = if is_patch {
        " Edit Snippet  (Tab: next field | F10: save | Esc: cancel) "
    } else {
        " New Snippet  (Tab/Enter: next field | F10: submit | Esc: cancel) "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(teal())
        .title(Span::styled(title, bold_teal()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(8),
        ])
        .split(inner);

    let form = if is_patch {
        &mut app.snippets.patch_form
    } else {
        &mut app.snippets.submit_form
    };
    for (i, chunk) in chunks.iter().enumerate() {
        if i < form.fields.len() {
            f.render_widget(&form.fields[i], *chunk);
        }
    }

    if let Some(e) = &(if is_patch {
        &app.snippets.error
    } else {
        &app.snippets.error
    })
    .clone()
    {
        let popup = centered_rect(50, 15, area);
        f.render_widget(Clear, popup);
        let p = Paragraph::new(format!("  {}", e))
            .style(Style::default().fg(RED))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(RED)),
            );
        f.render_widget(p, popup);
    }
}

fn render_lists(app: &mut App, f: &mut Frame, area: Rect) {
    if app.lists.mode == ListsMode::CreateForm {
        render_list_create_form(app, f, area);
        return;
    }

    if app.lists.mode == ListsMode::Detail {
        render_list_detail(app, f, area);
        if app.lists.mode == ListsMode::AddSnippet {
            let popup = centered_rect(40, 20, area);
            f.render_widget(Clear, popup);
            f.render_widget(&app.lists.add_snippet_input, popup);
        }
        return;
    }

    if let Some(e) = &app.lists.error.clone() {
        let p = Paragraph::new(format!("  Error: {}", e))
            .style(Style::default().fg(RED))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(teal())
                    .title(Span::styled(" Lists ", bold_teal())),
            );
        f.render_widget(p, area);
        return;
    }

    if let Some(s) = &app.lists.success.clone() {
        let popup = centered_rect(50, 10, area);
        f.render_widget(Clear, popup);
        let p = Paragraph::new(format!("  {}", s))
            .style(Style::default().fg(GREEN))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(GREEN)),
            );
        f.render_widget(p, popup);
    }

    if app.lists.loading {
        let p = Paragraph::new("  Loading…").style(dim()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(teal())
                .title(Span::styled(" Lists ", bold_teal())),
        );
        f.render_widget(p, area);
        return;
    }

    if app.lists.my_lists.is_empty() {
        let msg = if app.api.token.is_none() {
            "  Not authenticated. Set MICROCODES_API_TOKEN to view your lists."
        } else {
            "  No lists found. Press n to create one."
        };
        let p = Paragraph::new(msg).style(dim()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(teal())
                .title(Span::styled(" Lists ", bold_teal())),
        );
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("ID").style(header_style()),
        Cell::from("Title").style(header_style()),
        Cell::from("Snippets").style(header_style()),
        Cell::from("Unlisted").style(header_style()),
    ])
    .height(1)
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .lists
        .my_lists
        .iter()
        .map(|l| {
            let count = l
                .get("snippets")
                .and_then(|s| s.as_array())
                .map(|a| a.len().to_string())
                .unwrap_or_else(|| nv(l, "snippetCount"));
            let unlisted = l
                .get("unlisted")
                .and_then(|v| v.as_bool())
                .map(|b| if b { "yes" } else { "no" })
                .unwrap_or("no");
            Row::new(vec![
                Cell::from(truncate(&sv(l, "id"), 24)),
                Cell::from(truncate(&sv(l, "title"), 42)),
                Cell::from(count),
                Cell::from(unlisted),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(26),
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(teal())
            .title(Span::styled(" My Lists ", bold_teal())),
    )
    .row_highlight_style(selected())
    .highlight_symbol("▶ ");

    f.render_stateful_widget(table, area, &mut app.lists.table_state);

    if let ListsMode::Confirm(ref kind) = app.lists.mode.clone() {
        render_confirm_popup(f, area, confirm_message(kind));
    }
}

fn render_list_detail(app: &mut App, f: &mut Frame, area: Rect) {
    let detail = match &app.lists.detail {
        Some(d) => d.clone(),
        None => return,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(0)])
        .split(area);

    let unlisted = detail
        .get("unlisted")
        .and_then(|v| v.as_bool())
        .map(|b| if b { "yes" } else { "no" })
        .unwrap_or("no");
    let info_lines = vec![
        Line::from(vec![
            Span::styled("  Title:        ", header_style()),
            Span::raw(sv(&detail, "title")),
        ]),
        Line::from(vec![
            Span::styled("  ID:           ", header_style()),
            Span::raw(sv(&detail, "id")),
        ]),
        Line::from(vec![
            Span::styled("  Description:  ", header_style()),
            Span::raw(sv(&detail, "description")),
        ]),
        Line::from(vec![
            Span::styled("  Unlisted:     ", header_style()),
            Span::raw(unlisted),
        ]),
    ];
    let info = Paragraph::new(info_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(teal())
            .title(Span::styled(
                " List Detail  (a:Add snippet | Esc:Back) ",
                bold_teal(),
            )),
    );
    f.render_widget(info, chunks[0]);

    let snippets = detail
        .get("snippets")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();

    if snippets.is_empty() {
        let p = Paragraph::new("  No snippets in this list yet. Press a to add one.")
            .style(dim())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(teal())
                    .title(Span::styled(" Snippets ", bold_teal())),
            );
        f.render_widget(p, chunks[1]);
        return;
    }

    let header = Row::new(vec![
        Cell::from("ID").style(header_style()),
        Cell::from("Title").style(header_style()),
        Cell::from("Language").style(header_style()),
    ])
    .height(1)
    .bottom_margin(1);

    let rows: Vec<Row> = snippets
        .iter()
        .map(|s| {
            Row::new(vec![
                Cell::from(truncate(&sv(s, "id"), 24)),
                Cell::from(truncate(&sv(s, "title"), 42)),
                Cell::from(sv(s, "language")),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(26),
            Constraint::Min(20),
            Constraint::Length(14),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(teal())
            .title(Span::styled(" Snippets in List ", bold_teal())),
    )
    .row_highlight_style(selected())
    .highlight_symbol("▶ ");

    f.render_stateful_widget(table, chunks[1], &mut app.lists.detail_table);
}

fn render_list_create_form(app: &mut App, f: &mut Frame, area: Rect) {
    let unlisted_label = if app.lists.create_form.unlisted {
        "Unlisted: YES"
    } else {
        "Unlisted: NO"
    };
    let title = format!(
        " New List  (Tab: next field | Ctrl+u: {} | F10: create | Esc: cancel) ",
        unlisted_label
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(teal())
        .title(Span::styled(title, bold_teal()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(0),
        ])
        .split(inner);

    f.render_widget(&app.lists.create_form.fields[0], chunks[0]);
    f.render_widget(&app.lists.create_form.fields[1], chunks[1]);

    if let Some(e) = &app.lists.error.clone() {
        let p = Paragraph::new(format!("  Error: {}", e)).style(Style::default().fg(RED));
        f.render_widget(p, chunks[2]);
    }
}

fn render_requests(app: &mut App, f: &mut Frame, area: Rect) {
    if app.requests.mode == RequestsMode::SubmitForm {
        render_request_form(app, f, area);
        return;
    }

    if app.requests.mode == RequestsMode::Detail {
        render_request_detail(app, f, area);
        return;
    }

    if let Some(e) = &app.requests.error.clone() {
        let p = Paragraph::new(format!("  Error: {}", e))
            .style(Style::default().fg(RED))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(teal())
                    .title(Span::styled(" Requests ", bold_teal())),
            );
        f.render_widget(p, area);
        return;
    }

    if app.requests.loading {
        let p = Paragraph::new("  Loading…").style(dim()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(teal())
                .title(Span::styled(" Requests ", bold_teal())),
        );
        f.render_widget(p, area);
        return;
    }

    if app.requests.requests.is_empty() {
        let p = Paragraph::new("  No requests found. Press n to submit one.")
            .style(dim())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(teal())
                    .title(Span::styled(" Requests ", bold_teal())),
            );
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("ID").style(header_style()),
        Cell::from("Title").style(header_style()),
        Cell::from("Status").style(header_style()),
        Cell::from("Submitter").style(header_style()),
    ])
    .height(1)
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .requests
        .requests
        .iter()
        .map(|r| {
            let status = sv(r, "status");
            let status_color = match status.as_str() {
                "open" => GREEN,
                "fulfilled" => TEAL,
                "closed" | "rejected" => RED,
                _ => FG,
            };
            Row::new(vec![
                Cell::from(truncate(&sv(r, "id"), 24)),
                Cell::from(truncate(&sv(r, "title"), 36)),
                Cell::from(status).style(Style::default().fg(status_color)),
                Cell::from(sv(r, "submitter")),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(26),
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Length(16),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(teal())
            .title(Span::styled(" Requests ", bold_teal())),
    )
    .row_highlight_style(selected())
    .highlight_symbol("▶ ");

    f.render_stateful_widget(table, area, &mut app.requests.table_state);

    if let RequestsMode::Confirm(ref kind) = app.requests.mode.clone() {
        render_confirm_popup(f, area, confirm_message(kind));
    }
}

fn render_commands(app: &mut App, f: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    f.render_widget(&app.commands.input, chunks[0]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(teal())
        .title(Span::styled(" CLI Command Output ", bold_teal()));
    let inner = block.inner(chunks[1]);
    f.render_widget(block, chunks[1]);

    let output_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(inner);

    let mut header_lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "  Run any microcodes CLI command here (same command surface as shell).",
            dim(),
        )),
        Line::from(Span::styled(
            "  Examples: search \"redis\" --sort newest   |   requests --status open --limit 10",
            dim(),
        )),
        Line::from(Span::styled(
            "  Prefix 'microcodes' or 'mcodes' is optional.",
            dim(),
        )),
    ];
    if let Some(s) = &app.commands.success {
        header_lines.push(Line::from(Span::styled(
            format!("  {}", s),
            Style::default().fg(GREEN),
        )));
    }
    if let Some(e) = &app.commands.error {
        header_lines.push(Line::from(Span::styled(
            format!("  {}", e),
            Style::default().fg(RED),
        )));
    }
    if app.commands.loading {
        header_lines.push(Line::from(Span::styled(
            "  Running…",
            Style::default().fg(YELLOW),
        )));
    }

    let header = Paragraph::new(header_lines);
    f.render_widget(header, output_chunks[0]);

    let body = if app.commands.output.trim().is_empty() {
        "No output yet. Enter a command and press Enter.".to_string()
    } else {
        app.commands.output.clone()
    };

    let out = Paragraph::new(body)
        .wrap(Wrap { trim: false })
        .scroll((app.commands.scroll, 0))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(DARK_TEAL)),
        );
    f.render_widget(out, output_chunks[1]);
}

fn render_request_detail(app: &mut App, f: &mut Frame, area: Rect) {
    let detail = match &app.requests.detail {
        Some(d) => d.clone(),
        None => return,
    };

    let tags = detail
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    let status = sv(&detail, "status");
    let status_color = match status.as_str() {
        "open" => GREEN,
        "fulfilled" => TEAL,
        _ => RED,
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("  Title:        ", header_style()),
            Span::raw(sv(&detail, "title")),
        ]),
        Line::from(vec![
            Span::styled("  ID:           ", header_style()),
            Span::raw(sv(&detail, "id")),
        ]),
        Line::from(vec![
            Span::styled("  Status:       ", header_style()),
            Span::styled(status, Style::default().fg(status_color)),
        ]),
        Line::from(vec![
            Span::styled("  Submitter:    ", header_style()),
            Span::raw(sv(&detail, "submitter")),
        ]),
        Line::from(vec![
            Span::styled("  Tags:         ", header_style()),
            Span::raw(tags),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Description:", bold_teal())),
        Line::from(""),
        Line::from(format!("    {}", sv(&detail, "description"))),
    ];

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(teal())
                .title(Span::styled(
                    " Request Detail  (Esc to go back) ",
                    bold_teal(),
                )),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn render_request_form(app: &mut App, f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(teal())
        .title(Span::styled(
            " New Request  (Tab/Enter: next field | F10: submit | Esc: cancel) ",
            bold_teal(),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(inner);

    f.render_widget(&app.requests.submit_form.fields[0], chunks[0]);
    f.render_widget(&app.requests.submit_form.fields[1], chunks[1]);
    f.render_widget(&app.requests.submit_form.fields[2], chunks[2]);

    if let Some(e) = &app.requests.error.clone() {
        let popup = centered_rect(50, 15, area);
        f.render_widget(Clear, popup);
        let p = Paragraph::new(format!("  {}", e))
            .style(Style::default().fg(RED))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(RED)),
            );
        f.render_widget(p, popup);
    }
}

fn confirm_message(kind: &ConfirmKind) -> &str {
    match kind {
        ConfirmKind::DeleteSnippet(_) => "Delete this snippet? (y/N)",
        ConfirmKind::DeleteList(_) => "Delete this list? (y/N)",
        ConfirmKind::DeleteRequest(_) => "Delete this request? (y/N)",
        ConfirmKind::DeleteComment(_) => "Delete this comment? (y/N)",
    }
}

fn render_confirm_popup(f: &mut Frame, area: Rect, message: &str) {
    let popup = centered_rect(40, 15, area);
    f.render_widget(Clear, popup);
    let p = Paragraph::new(format!("  {}", message))
        .style(Style::default().fg(YELLOW))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(YELLOW))
                .title(Span::styled(
                    " Confirm ",
                    Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
                )),
        );
    f.render_widget(p, popup);
}

fn render_help(f: &mut Frame, area: Rect) {
    let popup = centered_rect(60, 80, area);
    f.render_widget(Clear, popup);

    let lines = vec![
        Line::from(Span::styled("  Global", bold_teal())),
        Line::from("  1-5           Switch tabs"),
        Line::from("  Ctrl+C / q   Quit"),
        Line::from("  ?             Toggle this help"),
        Line::from(""),
        Line::from(Span::styled("  Snippets", bold_teal())),
        Line::from("  /             Focus search"),
        Line::from("  Enter         Focus search / run in search bar"),
        Line::from("  id:<id1,id2>  Fetch specific snippet IDs"),
        Line::from("  f             Open filters"),
        Line::from("  s             Open sort options"),
        Line::from("  ← / →         Previous / next page"),
        Line::from("  ↑↓ / j k      Navigate results"),
        Line::from("  Enter         Open actions menu"),
        Line::from("  n             New snippet (source picker)"),
        Line::from("                Sources: file, folder, stdin, form"),
        Line::from("  Tab           Next form field"),
        Line::from("  Enter         Next field (code field inserts newline)"),
        Line::from("  F10           Submit / save form"),
        Line::from("  Esc           Cancel / go back"),
        Line::from(""),
        Line::from(Span::styled("  Filters", bold_teal())),
        Line::from("  Tab/BackTab   Move between fields"),
        Line::from("  Enter         Next field (last field: apply)"),
        Line::from("  F10           Apply filters"),
        Line::from("  Esc           Cancel"),
        Line::from(""),
        Line::from(Span::styled("  Lists", bold_teal())),
        Line::from("  ↑↓ / j k      Navigate"),
        Line::from("  Enter         View list"),
        Line::from("  n             New list"),
        Line::from("  d             Delete selected list"),
        Line::from("  a (in detail) Add snippet to list"),
        Line::from("  r             Refresh"),
        Line::from(""),
        Line::from(Span::styled("  Requests", bold_teal())),
        Line::from("  ↑↓ / j k      Navigate"),
        Line::from("  Enter         View request"),
        Line::from("  n             New request"),
        Line::from("  d             Delete selected request"),
        Line::from("  r             Refresh"),
        Line::from(""),
        Line::from(Span::styled("  Commands", bold_teal())),
        Line::from("  Enter         Run command"),
        Line::from("  Esc           Unfocus input"),
        Line::from("  / or i        Focus input"),
        Line::from("  Ctrl+↑ / Ctrl+↓  Command history"),
        Line::from("  r             Rerun last command"),
        Line::from("  c             Clear output"),
        Line::from("  ↑↓ / j k      Scroll output"),
        Line::from(""),
        Line::from(Span::styled("  Press Esc or ? to close this help", dim())),
    ];

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(bold_teal())
                .title(Span::styled(" Help ", bold_teal())),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(p, popup);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn run_tui(base_url: String, token: Option<String>) -> Result<(), String> {
    enable_raw_mode().map_err(|e| format!("Terminal error: {}", e))?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| format!("Terminal error: {}", e))?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend).map_err(|e| format!("Terminal error: {}", e))?;

    let api = ApiConfig { base_url, token };
    let mut app = App::new(api);

    let tick = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    let result = loop {
        if let Err(e) = terminal.draw(|f| render(&mut app, f)) {
            break Err(format!("Draw error: {}", e));
        }

        let timeout = tick
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::ZERO);
        if event::poll(timeout).map_err(|e| format!("Event error: {}", e))? {
            match event::read().map_err(|e| format!("Event error: {}", e))? {
                Event::Key(key) => app.handle_key(key),
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick {
            app.poll_messages();
            last_tick = Instant::now();
        }

        if app.should_quit {
            break Ok(());
        }
    };

    disable_raw_mode().ok();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .ok();
    terminal.show_cursor().ok();

    result
}