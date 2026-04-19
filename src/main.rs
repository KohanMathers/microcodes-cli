mod tui;

use clap::{Args, Parser, Subcommand, ValueEnum};
use colored::Colorize;
use reqwest::blocking::Client as HttpClient;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, IsTerminal, Read, Write as IoWrite};
use std::path::Path;
use std::process;

const DEFAULT_BASE_URL: &str = "https://micro.codes";

#[derive(Parser)]
#[command(
    name = "microcodes",
    about = "Interact with the Microcodes API from the terminal",
    version,
    propagate_version = true,
    subcommand_required = false,
    arg_required_else_help = false
)]
struct Cli {
    /// Output raw JSON instead of formatted output
    #[arg(long, global = true)]
    json: bool,

    /// Suppress colour and formatting
    #[arg(long, global = true)]
    plain: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Search for snippets
    Search(SearchArgs),

    /// Fetch a single snippet by ID
    Get {
        /// Snippet ID
        id: String,
    },

    /// Fetch multiple snippets by comma-separated IDs
    Ids {
        /// Comma-separated snippet IDs
        ids: String,
    },

    /// List your snippets (authenticated)
    #[command(name = "my-snippets")]
    MySnippets,

    /// Submit a snippet (authenticated)
    Submit(SubmitArgs),

    /// Delete a snippet (authenticated)
    Delete(DeleteArgs),

    /// Patch a snippet field (authenticated)
    Patch(PatchArgs),

    /// List version history of a snippet
    Versions {
        /// Snippet ID
        id: String,
    },

    /// Show diff between two versions of a snippet
    Diff(DiffArgs),

    /// Show snippet metrics (upvotes, downvotes, bookmarks)
    Metrics {
        /// Snippet ID
        id: String,
    },

    /// Vote on a snippet (authenticated)
    Vote {
        /// Snippet ID
        id: String,
        /// Vote direction
        direction: VoteDirection,
    },

    /// Toggle bookmark on a snippet (authenticated)
    Bookmark {
        /// Snippet ID
        id: String,
        /// Remove the bookmark instead of adding it
        #[arg(long)]
        remove: bool,
    },

    /// Print the JSON schema for snippet submission
    Schema,

    /// Render markdown to HTML
    Markdown(MarkdownArgs),

    /// Comment operations
    Comment(CommentArgs),

    /// List operations
    Lists(ListsArgs),

    /// Snippet request operations
    Requests(RequestsArgs),

    /// Show current user profile (authenticated)
    Whoami,

    /// Session operations (authenticated)
    Sessions(SessionsArgs),

    /// Passkey operations (authenticated)
    Passkeys(PasskeysArgs),

    /// Change your username (authenticated)
    Username {
        /// New username
        username: String,
    },

    /// Update your bio (authenticated)
    Bio {
        /// New bio text
        bio: String,
    },

    /// Update privacy settings (authenticated)
    Privacy(PrivacyArgs),

    /// Unlink an OAuth provider (authenticated)
    Unlink {
        /// Provider name: github, google, or gitlab
        provider: String,
    },

    /// Delete your account — cannot be undone (authenticated)
    #[command(name = "delete-account")]
    DeleteAccount,

    /// Check service health
    Health,

    /// Report a snippet, user, or comment (authenticated)
    Report {
        /// Content type: snippet, user, or comment
        kind: String,
        /// Content ID
        id: String,
        /// Reason for the report
        reason: String,
    },

    /// Send feedback (authenticated)
    Feedback {
        /// Feedback message
        message: String,
    },

    /// Check for a newer release and update the binary if available
    Update,

    /// Save your API token to the shell environment
    Token {
        /// Your Microcodes API token
        token: String,
    },
}

#[derive(Args)]
struct SearchArgs {
    /// Search query
    query: String,
    /// Filter by tags (prefix with ! to exclude, comma-separated)
    #[arg(long)]
    tags: Option<String>,
    /// Filter by language (comma-separated)
    #[arg(long)]
    languages: Option<String>,
    /// Filter by submitter username (use 'me' for own snippets)
    #[arg(long)]
    submitter: Option<String>,
    /// Filter by AI-generated status: include, exclude, or only
    #[arg(long)]
    generated: Option<String>,
    /// Sort order: relevance, oldest, newest, upvotes
    #[arg(long)]
    sort: Option<String>,
    /// Page number
    #[arg(long)]
    page: Option<u32>,
}

#[derive(Args)]
struct SubmitArgs {
    /// Path to snippet JSON file (reads from stdin if omitted)
    #[arg(long)]
    file: Option<String>,
    /// Path to a folder containing meta.json and source files
    #[arg(long)]
    folder: Option<String>,
}

#[derive(Args)]
struct DeleteArgs {
    /// Snippet ID
    id: String,
    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    yes: bool,
}

#[derive(Args)]
struct PatchArgs {
    /// Snippet ID
    id: String,
    /// New title
    #[arg(long)]
    title: Option<String>,
    /// New description
    #[arg(long)]
    description: Option<String>,
    /// Arbitrary field update in key=value format (may be repeated)
    #[arg(long)]
    field: Vec<String>,
}

#[derive(Args)]
struct DiffArgs {
    /// Snippet ID
    id: String,
    /// From version number
    #[arg(long)]
    from: u32,
    /// To version number
    #[arg(long)]
    to: u32,
}

#[derive(ValueEnum, Clone, Debug)]
enum VoteDirection {
    Up,
    Down,
    Remove,
}

impl VoteDirection {
    fn as_str(&self) -> &str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Remove => "remove",
        }
    }
}

#[derive(Args)]
struct MarkdownArgs {
    /// Path to markdown file (reads from stdin if omitted)
    #[arg(long)]
    file: Option<String>,
}

#[derive(Args)]
struct CommentArgs {
    #[command(subcommand)]
    action: CommentAction,
}

#[derive(Subcommand)]
enum CommentAction {
    /// Add a comment to a snippet (authenticated)
    Add(CommentAddArgs),
    /// Vote on a comment (authenticated)
    Vote(CommentVoteArgs),
    /// Edit a comment (authenticated)
    Edit(CommentEditArgs),
    /// Delete a comment (authenticated)
    Delete(CommentDeleteArgs),
}

#[derive(Args)]
struct CommentAddArgs {
    /// Snippet ID to comment on
    snippet_id: String,
    /// Comment text
    text: String,
    /// Reply to comment ID
    #[arg(long)]
    reply_to: Option<String>,
}

#[derive(Args)]
struct CommentVoteArgs {
    /// Comment ID
    comment_id: String,
    /// Vote direction
    direction: VoteDirection,
}

#[derive(Args)]
struct CommentEditArgs {
    /// Comment ID
    comment_id: String,
    /// New comment text
    text: String,
}

#[derive(Args)]
struct CommentDeleteArgs {
    /// Comment ID
    comment_id: String,
    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    yes: bool,
}

#[derive(Args)]
struct ListsArgs {
    #[command(subcommand)]
    action: Option<ListsAction>,
}

#[derive(Subcommand)]
enum ListsAction {
    /// Create a new list (authenticated)
    Create(ListCreateArgs),
    /// Get a list by ID
    Get {
        /// List ID
        id: String,
    },
    /// Get public lists for a user
    User {
        /// User ID
        user_id: String,
    },
    /// Update a list (authenticated)
    Update(ListUpdateArgs),
    /// Delete a list (authenticated)
    Delete(ListDeleteArgs),
    /// Add a snippet to a list (authenticated)
    Add {
        /// List ID
        list_id: String,
        /// Snippet ID
        snippet_id: String,
    },
    /// Remove a snippet from a list (authenticated)
    Remove {
        /// List ID
        list_id: String,
        /// Snippet ID
        snippet_id: String,
    },
}

#[derive(Args)]
struct ListCreateArgs {
    /// List title
    title: String,
    /// List description
    #[arg(long)]
    description: Option<String>,
    /// Make the list unlisted (not public)
    #[arg(long)]
    unlisted: bool,
}

#[derive(Args)]
struct ListUpdateArgs {
    /// List ID
    id: String,
    /// New title
    #[arg(long)]
    title: Option<String>,
    /// New description
    #[arg(long)]
    description: Option<String>,
    /// Set unlisted status
    #[arg(long)]
    unlisted: Option<bool>,
}

#[derive(Args)]
struct ListDeleteArgs {
    /// List ID
    id: String,
    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    yes: bool,
}

#[derive(Args)]
struct RequestsArgs {
    #[command(subcommand)]
    action: Option<RequestsAction>,
    /// Filter by status: open, fulfilled, closed, rejected
    #[arg(long)]
    status: Option<String>,
    /// Filter by user ID
    #[arg(long)]
    user: Option<String>,
    /// Limit number of results
    #[arg(long)]
    limit: Option<u32>,
    /// Offset for pagination
    #[arg(long)]
    offset: Option<u32>,
}

#[derive(Subcommand)]
enum RequestsAction {
    /// Get a request by ID
    Get {
        /// Request ID
        id: String,
    },
    /// Submit a new snippet request (authenticated)
    Submit(RequestSubmitArgs),
    /// Delete a request (authenticated)
    Delete(RequestDeleteArgs),
    /// Update request status (authenticated)
    Status(RequestStatusArgs),
    /// Fulfill a request with a snippet (authenticated)
    Fulfill {
        /// Request ID
        request_id: String,
        /// Snippet ID that fulfills the request
        snippet_id: String,
    },
}

#[derive(Args)]
struct RequestSubmitArgs {
    /// Request title
    title: String,
    /// Request description
    description: String,
    /// Comma-separated tags
    #[arg(long)]
    tags: Option<String>,
}

#[derive(Args)]
struct RequestDeleteArgs {
    /// Request ID
    id: String,
    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    yes: bool,
}

#[derive(Args)]
struct RequestStatusArgs {
    /// Request ID
    id: String,
    /// New status: open, fulfilled, closed, rejected
    status: String,
}

#[derive(Args)]
struct SessionsArgs {
    #[command(subcommand)]
    action: Option<SessionsAction>,
}

#[derive(Subcommand)]
enum SessionsAction {
    /// Disconnect all active sessions (authenticated)
    Disconnect {
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Args)]
struct PasskeysArgs {
    #[command(subcommand)]
    action: Option<PasskeysAction>,
}

#[derive(Subcommand)]
enum PasskeysAction {
    /// Rename a passkey (authenticated)
    Rename {
        /// Credential ID
        credential_id: String,
        /// New label
        label: String,
    },
    /// Delete a passkey (authenticated)
    Delete {
        /// Credential ID
        credential_id: String,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Args)]
struct PrivacyArgs {
    /// Profile visibility: public, unlisted, or private
    #[arg(long)]
    visibility: Option<String>,
    /// Show bio on profile
    #[arg(long)]
    show_bio: Option<bool>,
    /// Show snippets on profile
    #[arg(long)]
    show_snippets: Option<bool>,
    /// Show lists on profile
    #[arg(long)]
    show_lists: Option<bool>,
    /// Show comments on profile
    #[arg(long)]
    show_comments: Option<bool>,
    /// Default list visibility: public or unlisted
    #[arg(long)]
    default_list_visibility: Option<String>,
}

struct Context {
    base_url: String,
    token: Option<String>,
    json_output: bool,
    http: HttpClient,
}

impl Context {
    fn token_or_error(&self) -> Result<String, String> {
        self.token.clone().ok_or_else(|| {
            "MICROCODES_API_TOKEN is not set.\nExport it with: export MICROCODES_API_TOKEN=your_key_here"
                .to_string()
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn send(&self, req: reqwest::blocking::RequestBuilder, url: &str) -> Result<Value, String> {
        let resp = req
            .send()
            .map_err(|e| format!("Connection error ({}): {}", url, e))?;
        handle_response(resp)
    }

    fn get(&self, path: &str) -> Result<Value, String> {
        let url = self.url(path);
        self.send(self.http.get(&url), &url)
    }

    fn get_q(&self, path: &str, params: &[(&str, String)]) -> Result<Value, String> {
        let mut url = self.url(path);
        if !params.is_empty() {
            let qs: Vec<String> = params.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
            url = format!("{}?{}", url, qs.join("&"));
        }
        self.send(self.http.get(&url), &url)
    }

    fn auth_get(&self, path: &str) -> Result<Value, String> {
        let token = self.token_or_error()?;
        let url = self.url(path);
        self.send(self.http.get(&url).header("X-API-Key", token), &url)
    }

    fn auth_get_q(&self, path: &str, params: &[(&str, String)]) -> Result<Value, String> {
        let token = self.token_or_error()?;
        let mut url = self.url(path);
        if !params.is_empty() {
            let qs: Vec<String> = params.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
            url = format!("{}?{}", url, qs.join("&"));
        }
        self.send(self.http.get(&url).header("X-API-Key", token), &url)
    }

    fn post(&self, path: &str, body: Value) -> Result<Value, String> {
        let url = self.url(path);
        self.send(self.http.post(&url).json(&body), &url)
    }

    fn auth_post(&self, path: &str, body: Value) -> Result<Value, String> {
        let token = self.token_or_error()?;
        let url = self.url(path);
        self.send(
            self.http.post(&url).header("X-API-Key", token).json(&body),
            &url,
        )
    }

    fn auth_delete(&self, path: &str) -> Result<Value, String> {
        let token = self.token_or_error()?;
        let url = self.url(path);
        self.send(self.http.delete(&url).header("X-API-Key", token), &url)
    }

    fn auth_put(&self, path: &str, body: Value) -> Result<Value, String> {
        let token = self.token_or_error()?;
        let url = self.url(path);
        self.send(
            self.http.put(&url).header("X-API-Key", token).json(&body),
            &url,
        )
    }

    fn auth_patch(&self, path: &str, body: Value) -> Result<Value, String> {
        let token = self.token_or_error()?;
        let url = self.url(path);
        self.send(
            self.http.patch(&url).header("X-API-Key", token).json(&body),
            &url,
        )
    }
}

fn handle_response(resp: reqwest::blocking::Response) -> Result<Value, String> {
    let status = resp.status();
    let text = resp
        .text()
        .map_err(|e| format!("Failed to read response body: {}", e))?;

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
        403 => "Forbidden. You don't have permission to do that.".to_string(),
        404 => "Not found.".to_string(),
        code => format!("HTTP {}: {}", code, msg),
    })
}

fn print_json(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn str_val(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

fn num_val(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_i64())
        .map(|n| n.to_string())
        .unwrap_or_default()
}

fn fmt_ts(v: &Value, key: &str) -> String {
    let ms = match v.get(key).and_then(|x| x.as_i64()) {
        Some(ms) => ms,
        None => return String::new(),
    };
    let secs = ms / 1000;
    let days = secs / 86400;
    let mut y = 1970u32;
    let mut rem = days as u32;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if rem < days_in_year {
            break;
        }
        rem -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [
        31u32,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0usize;
    for &md in &month_days {
        if rem < md {
            break;
        }
        rem -= md;
        m += 1;
    }
    let day = rem + 1;
    let hh = (secs % 86400) / 3600;
    let mm = (secs % 3600) / 60;
    format!("{y}-{:02}-{:02} {:02}:{:02}", m + 1, day, hh, mm)
}

fn uuid7_created(id: &str) -> String {
    let hex: String = id.chars().filter(|c| *c != '-').collect();
    if hex.len() < 12 {
        return String::new();
    }
    let ms = match u64::from_str_radix(&hex[..12], 16) {
        Ok(v) => v as i64,
        Err(_) => return String::new(),
    };
    let secs = ms / 1000;
    let days = secs / 86400;
    let mut y = 1970u32;
    let mut rem = days as u32;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if rem < days_in_year {
            break;
        }
        rem -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [
        31u32,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut m = 0usize;
    for &md in &month_days {
        if rem < md {
            break;
        }
        rem -= md;
        m += 1;
    }
    let day = rem + 1;
    let hh = (secs % 86400) / 3600;
    let mm = (secs % 3600) / 60;
    format!("{y}-{:02}-{:02} {:02}:{:02} UTC", m + 1, day, hh, mm)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{}...", cut)
    }
}

fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        println!("  (no results)");
        return;
    }
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }
    let header_parts: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{:<width$}", h, width = widths[i]))
        .collect();
    println!("  {}", header_parts.join("  ").bold().cyan());
    for row in rows {
        let row_parts: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let w = widths.get(i).copied().unwrap_or(0);
                format!("{:<width$}", cell, width = w)
            })
            .collect();
        println!("  {}", row_parts.join("  "));
    }
}

fn print_detail(fields: &[(&str, String)]) {
    let label_w = fields.iter().map(|(k, _)| k.len()).max().unwrap_or(0) + 2;
    for (key, value) in fields {
        let label = format!("{:<width$}", key, width = label_w);
        println!("  {}  {}", label.bold(), value);
    }
}

fn print_success(msg: &str) {
    println!("{}", format!("✓ {}", msg).green());
}

fn confirm(prompt: &str) -> bool {
    print!("{} [y/N] ", prompt);
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap_or(0);
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

fn read_stdin_or_file(file: Option<&str>) -> Result<String, String> {
    if let Some(path) = file {
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read file '{}': {}", path, e))
    } else {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("Failed to read stdin: {}", e))?;
        Ok(buf)
    }
}

fn snippet_rows(arr: &[Value]) -> Vec<Vec<String>> {
    arr.iter()
        .map(|s| {
            vec![
                str_val(s, "id"),
                truncate(&str_val(s, "title"), 40),
                str_val(s, "language"),
                num_val(s, "upvotes"),
            ]
        })
        .collect()
}

fn print_snippet_table(arr: &[Value]) {
    let rows = snippet_rows(arr);
    print_table(&["ID", "TITLE", "LANGUAGE", "UPVOTES"], &rows);
}

fn print_snippet_detail(s: &Value) {
    let tags = s
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    print_detail(&[
        ("Title", str_val(s, "title")),
        ("ID", str_val(s, "id")),
        ("Language", str_val(s, "language")),
        ("Description", str_val(s, "description")),
        ("Tags", tags),
        ("Submitter", str_val(s, "submitter")),
        ("Upvotes", num_val(s, "upvotes")),
        ("Downvotes", num_val(s, "downvotes")),
        ("Bookmarks", num_val(s, "bookmarks")),
        ("Created", str_val(s, "createdAt")),
    ]);
    if let Some(code) = s.get("code").and_then(|v| v.as_str()) {
        println!();
        println!("  {}", "Code:".bold());
        for line in code.lines() {
            println!("    {}", line);
        }
    }
}

fn cmd_search(args: SearchArgs, ctx: &Context) -> Result<(), String> {
    let mut params: Vec<(&str, String)> = vec![("q", args.query)];
    if let Some(v) = args.tags {
        params.push(("tags", v));
    }
    if let Some(v) = args.languages {
        params.push(("languages", v));
    }
    if let Some(v) = args.submitter {
        params.push(("submitter", v));
    }
    if let Some(v) = args.generated {
        params.push(("generated", v));
    }
    if let Some(v) = args.sort {
        params.push(("sort", v));
    }
    if let Some(v) = args.page {
        params.push(("page", v.to_string()));
    }

    let data = ctx.get_q("/api/snippets", &params)?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    let snippets = data.as_array().cloned().unwrap_or_default();
    if snippets.is_empty() {
        println!("No results found.");
    } else {
        print_snippet_table(&snippets);
    }
    Ok(())
}

fn cmd_get(id: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.get(&format!("/api/snippets/{}", id))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    let snippet = if let Some(arr) = data.as_array() {
        arr.first().cloned().unwrap_or(Value::Null)
    } else {
        data
    };

    if snippet.is_null() {
        return Err("Not found.".to_string());
    }

    print_snippet_detail(&snippet);
    Ok(())
}

fn cmd_ids(ids: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.get_q("/api/snippets/by-ids", &[("ids", ids.to_string())])?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    let snippets = data.as_array().cloned().unwrap_or_default();
    print_snippet_table(&snippets);
    Ok(())
}

fn cmd_my_snippets(ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_get("/api/snippets/mine")?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    let snippets = data.as_array().cloned().unwrap_or_default();
    if snippets.is_empty() {
        println!("You have no snippets.");
    } else {
        print_snippet_table(&snippets);
    }
    Ok(())
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

fn cmd_submit(args: SubmitArgs, ctx: &Context) -> Result<(), String> {
    if args.file.is_some() && args.folder.is_some() {
        return Err("Cannot use both --file and --folder".to_string());
    }

    let payload: Value = if let Some(folder) = args.folder.as_deref() {
        build_folder_payload(folder)?
    } else {
        let raw = read_stdin_or_file(args.file.as_deref())?;
        serde_json::from_str(&raw).map_err(|e| format!("Invalid JSON: {}", e))?
    };

    match ctx.get("/api/schema") {
        Ok(schema) => {
            if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
                let missing: Vec<&str> = required
                    .iter()
                    .filter_map(|v| v.as_str())
                    .filter(|field| *field != "id")
                    .filter(|field| payload.get(field).map(|v| v.is_null()).unwrap_or(true))
                    .collect();
                if !missing.is_empty() {
                    return Err(format!("Missing required fields: {}", missing.join(", ")));
                }
            }
        }
        Err(e) => eprintln!("Warning: could not fetch schema for validation: {}", e),
    }

    let data = ctx.auth_post("/api/snippets", payload)?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success("Snippet submitted.");
    if let Some(id) = data.get("id").and_then(|v| v.as_str()) {
        println!("  ID: {}", id);
    }
    Ok(())
}

fn cmd_delete(args: DeleteArgs, ctx: &Context) -> Result<(), String> {
    if !args.yes {
        println!("About to delete snippet: {}", args.id.yellow());
        if !confirm("Are you sure?") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let data = ctx.auth_delete(&format!("/api/snippets/{}", args.id))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success(&format!("Snippet {} deleted.", args.id));
    Ok(())
}

fn cmd_patch(args: PatchArgs, ctx: &Context) -> Result<(), String> {
    let mut ops: Vec<Value> = Vec::new();

    if let Some(title) = args.title {
        ops.push(json!({"op": "replace", "path": "/title", "value": title}));
    }
    if let Some(desc) = args.description {
        ops.push(json!({"op": "replace", "path": "/description", "value": desc}));
    }
    for kv in &args.field {
        if let Some((k, v)) = kv.split_once('=') {
            ops.push(json!({"op": "replace", "path": format!("/{k}"), "value": v}));
        } else {
            return Err(format!(
                "Invalid --field format '{}': expected key=value",
                kv
            ));
        }
    }

    if ops.is_empty() {
        return Err(
            "No fields specified to patch. Use --title, --description, or --field key=value."
                .to_string(),
        );
    }

    let body = json!({"patch": ops});

    let data = ctx.auth_patch(&format!("/api/snippets/{}", args.id), body)?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success("Snippet updated.");
    Ok(())
}

fn cmd_versions(id: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_get(&format!("/api/snippets/{}/versions", id))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    let versions = data["versions"].as_array().cloned().unwrap_or_default();
    let rows: Vec<Vec<String>> = versions
        .iter()
        .map(|v| {
            vec![
                num_val(v, "version"),
                fmt_ts(v, "modified"),
                str_val(v, "editorUsername"),
            ]
        })
        .collect();
    print_table(&["VERSION", "MODIFIED", "EDITOR"], &rows);
    Ok(())
}

fn cmd_diff(args: DiffArgs, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_get_q(
        &format!("/api/snippets/{}/diff", args.id),
        &[("from", args.from.to_string()), ("to", args.to.to_string())],
    )?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    let from = data
        .get("fromVersion")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let to = data.get("toVersion").and_then(|v| v.as_i64()).unwrap_or(0);
    println!("diff v{} → v{}", from, to);

    let changes = data["changes"].as_array().cloned().unwrap_or_default();
    if changes.is_empty() {
        println!("No changes.");
    }
    for change in &changes {
        let path = change.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let op = change.get("op").and_then(|v| v.as_str()).unwrap_or("");
        println!("\n{} {}", op.yellow(), path.bold());
        let before = change.get("before").and_then(|v| v.as_str()).unwrap_or("");
        let after = change.get("after").and_then(|v| v.as_str()).unwrap_or("");
        for line in before.lines() {
            println!("{}", format!("- {}", line).red());
        }
        for line in after.lines() {
            println!("{}", format!("+ {}", line).green());
        }
    }
    Ok(())
}

fn cmd_metrics(id: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.get(&format!("/api/snippets/{}/metrics", id))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_detail(&[
        ("Upvotes", num_val(&data, "upvotes")),
        ("Downvotes", num_val(&data, "downvotes")),
        ("Bookmarks", num_val(&data, "bookmarks")),
    ]);
    Ok(())
}

fn cmd_vote(id: &str, direction: VoteDirection, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_put(
        &format!("/api/snippets/{}/vote", id),
        json!({"value": direction.as_str()}),
    )?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success(&format!("Voted {} on snippet {}.", direction.as_str(), id));
    Ok(())
}

fn cmd_bookmark(id: &str, value: bool, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_put(
        &format!("/api/snippets/{}/bookmark", id),
        json!({"value": value}),
    )?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    let msg = data
        .get("userBookmarked")
        .and_then(|v| v.as_bool())
        .map(|b| {
            if b {
                "Bookmarked."
            } else {
                "Bookmark removed."
            }
        })
        .unwrap_or("Done.");

    print_success(msg);
    Ok(())
}

fn cmd_schema(ctx: &Context) -> Result<(), String> {
    let data = ctx.get("/api/schema")?;
    print_json(&data);
    Ok(())
}

fn cmd_markdown(args: MarkdownArgs, ctx: &Context) -> Result<(), String> {
    let raw = read_stdin_or_file(args.file.as_deref())?;
    let data = ctx.post("/api/markdown", json!({"markdown": raw}))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    if let Some(html) = data.get("html").and_then(|v| v.as_str()) {
        println!("{}", html);
    } else if let Some(s) = data.as_str() {
        println!("{}", s);
    } else {
        print_json(&data);
    }
    Ok(())
}

fn cmd_comment(args: CommentArgs, ctx: &Context) -> Result<(), String> {
    match args.action {
        CommentAction::Add(a) => cmd_comment_add(a, ctx),
        CommentAction::Vote(a) => cmd_comment_vote(a, ctx),
        CommentAction::Edit(a) => cmd_comment_edit(a, ctx),
        CommentAction::Delete(a) => cmd_comment_delete(a, ctx),
    }
}

fn cmd_comment_add(args: CommentAddArgs, ctx: &Context) -> Result<(), String> {
    let mut body = json!({"comment": args.text});
    if let Some(reply_to) = args.reply_to {
        body["parentCommentId"] = json!(reply_to);
    }
    let data = ctx.auth_post(&format!("/api/snippets/{}/comments", args.snippet_id), body)?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success("Comment posted.");
    if let Some(id) = data.get("commentId").and_then(|v| v.as_str()) {
        println!("  Comment ID: {}", id);
    }
    Ok(())
}

fn cmd_comment_vote(args: CommentVoteArgs, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_put(
        &format!("/api/comments/{}/vote", args.comment_id),
        json!({"value": args.direction.as_str()}),
    )?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success(&format!(
        "Voted {} on comment {}.",
        args.direction.as_str(),
        args.comment_id
    ));
    Ok(())
}

fn cmd_comment_edit(args: CommentEditArgs, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_patch(
        &format!("/api/comments/{}", args.comment_id),
        json!({"comment": args.text}),
    )?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success("Comment updated.");
    Ok(())
}

fn cmd_comment_delete(args: CommentDeleteArgs, ctx: &Context) -> Result<(), String> {
    if !args.yes {
        println!("About to delete comment: {}", args.comment_id.yellow());
        if !confirm("Are you sure?") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let data = ctx.auth_delete(&format!("/api/comments/{}", args.comment_id))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success("Comment deleted.");
    Ok(())
}

fn cmd_lists(args: ListsArgs, ctx: &Context) -> Result<(), String> {
    match args.action {
        None => {
            let data = ctx.auth_get("/api/lists")?;
            if ctx.json_output {
                print_json(&data);
                return Ok(());
            }
            let lists = data.as_array().cloned().unwrap_or_default();
            if lists.is_empty() {
                println!("You have no lists.");
            } else {
                let rows: Vec<Vec<String>> = lists
                    .iter()
                    .map(|l| {
                        vec![
                            str_val(l, "id"),
                            truncate(&str_val(l, "title"), 40),
                            num_val(l, "snippetCount"),
                        ]
                    })
                    .collect();
                print_table(&["ID", "TITLE", "SNIPPETS"], &rows);
            }
            Ok(())
        }
        Some(ListsAction::Create(a)) => cmd_lists_create(a, ctx),
        Some(ListsAction::Get { id }) => cmd_lists_get(&id, ctx),
        Some(ListsAction::User { user_id }) => cmd_lists_user(&user_id, ctx),
        Some(ListsAction::Update(a)) => cmd_lists_update(a, ctx),
        Some(ListsAction::Delete(a)) => cmd_lists_delete(a, ctx),
        Some(ListsAction::Add {
            list_id,
            snippet_id,
        }) => cmd_lists_add(&list_id, &snippet_id, ctx),
        Some(ListsAction::Remove {
            list_id,
            snippet_id,
        }) => cmd_lists_remove(&list_id, &snippet_id, ctx),
    }
}

fn cmd_lists_create(args: ListCreateArgs, ctx: &Context) -> Result<(), String> {
    let mut body = json!({"title": args.title});
    if let Some(desc) = args.description {
        body["description"] = json!(desc);
    }
    if args.unlisted {
        body["unlisted"] = json!(true);
    }
    let data = ctx.auth_post("/api/lists", body)?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success("List created.");
    if let Some(id) = data.get("id").and_then(|v| v.as_str()) {
        println!("  ID: {}", id);
    }
    Ok(())
}

fn cmd_lists_get(id: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.get(&format!("/api/lists/{}", id))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_detail(&[
        ("Title", str_val(&data, "title")),
        ("ID", str_val(&data, "id")),
        ("Description", str_val(&data, "description")),
        ("Snippets", num_val(&data, "snippetCount")),
        ("Owner", str_val(&data, "owner")),
        ("Created", str_val(&data, "createdAt")),
    ]);

    if let Some(snippets) = data.get("snippets").and_then(|v| v.as_array()) {
        if !snippets.is_empty() {
            println!();
            print_snippet_table(snippets);
        }
    }
    Ok(())
}

fn cmd_lists_user(user_id: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.get(&format!("/api/lists/user/{}", user_id))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    let lists = data.as_array().cloned().unwrap_or_default();
    let rows: Vec<Vec<String>> = lists
        .iter()
        .map(|l| {
            vec![
                str_val(l, "id"),
                truncate(&str_val(l, "title"), 40),
                num_val(l, "snippetCount"),
            ]
        })
        .collect();
    print_table(&["ID", "TITLE", "SNIPPETS"], &rows);
    Ok(())
}

fn cmd_lists_update(args: ListUpdateArgs, ctx: &Context) -> Result<(), String> {
    let mut body = serde_json::Map::new();
    if let Some(v) = args.title {
        body.insert("title".to_string(), json!(v));
    }
    if let Some(v) = args.description {
        body.insert("description".to_string(), json!(v));
    }
    if let Some(v) = args.unlisted {
        body.insert("unlisted".to_string(), json!(v));
    }
    if body.is_empty() {
        return Err("No fields specified. Use --title, --description, or --unlisted.".to_string());
    }
    let data = ctx.auth_put(&format!("/api/lists/{}", args.id), Value::Object(body))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success("List updated.");
    Ok(())
}

fn cmd_lists_delete(args: ListDeleteArgs, ctx: &Context) -> Result<(), String> {
    if !args.yes {
        println!("About to delete list: {}", args.id.yellow());
        if !confirm("Are you sure?") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let data = ctx.auth_delete(&format!("/api/lists/{}", args.id))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success(&format!("List {} deleted.", args.id));
    Ok(())
}

fn cmd_lists_add(list_id: &str, snippet_id: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_post(
        &format!("/api/lists/{}/snippets", list_id),
        json!({"snippetId": snippet_id}),
    )?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success(&format!(
        "Snippet {} added to list {}.",
        snippet_id, list_id
    ));
    Ok(())
}

fn cmd_lists_remove(list_id: &str, snippet_id: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_delete(&format!("/api/lists/{}/snippets/{}", list_id, snippet_id))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success(&format!(
        "Snippet {} removed from list {}.",
        snippet_id, list_id
    ));
    Ok(())
}

fn cmd_requests(args: RequestsArgs, ctx: &Context) -> Result<(), String> {
    match args.action {
        None => {
            let mut params: Vec<(&str, String)> = vec![];
            if let Some(v) = args.status {
                params.push(("status", v));
            }
            if let Some(v) = args.user {
                params.push(("user", v));
            }
            if let Some(v) = args.limit {
                params.push(("limit", v.to_string()));
            }
            if let Some(v) = args.offset {
                params.push(("offset", v.to_string()));
            }

            let data = ctx.get_q("/api/requests", &params)?;

            if ctx.json_output {
                print_json(&data);
                return Ok(());
            }

            let reqs = data.as_array().cloned().unwrap_or_default();
            if reqs.is_empty() {
                println!("No requests found.");
            } else {
                let rows: Vec<Vec<String>> = reqs
                    .iter()
                    .map(|r| {
                        vec![
                            str_val(r, "id"),
                            truncate(&str_val(r, "title"), 40),
                            str_val(r, "status"),
                            str_val(r, "submitter"),
                        ]
                    })
                    .collect();
                print_table(&["ID", "TITLE", "STATUS", "SUBMITTER"], &rows);
            }
            Ok(())
        }
        Some(RequestsAction::Get { id }) => cmd_requests_get(&id, ctx),
        Some(RequestsAction::Submit(a)) => cmd_requests_submit(a, ctx),
        Some(RequestsAction::Delete(a)) => cmd_requests_delete(a, ctx),
        Some(RequestsAction::Status(a)) => cmd_requests_status(a, ctx),
        Some(RequestsAction::Fulfill {
            request_id,
            snippet_id,
        }) => cmd_requests_fulfill(&request_id, &snippet_id, ctx),
    }
}

fn cmd_requests_get(id: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.get(&format!("/api/requests/{}", id))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    let tags = data
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    print_detail(&[
        ("Title", str_val(&data, "title")),
        ("ID", str_val(&data, "id")),
        ("Status", str_val(&data, "status")),
        ("Description", str_val(&data, "description")),
        ("Tags", tags),
        ("Submitter", str_val(&data, "submitter")),
        ("Created", str_val(&data, "createdAt")),
    ]);
    Ok(())
}

fn cmd_requests_submit(args: RequestSubmitArgs, ctx: &Context) -> Result<(), String> {
    let mut body = json!({"title": args.title, "description": args.description});
    if let Some(tags) = args.tags {
        let tag_list: Vec<&str> = tags.split(',').map(|t| t.trim()).collect();
        body["tags"] = json!(tag_list);
    }
    let data = ctx.auth_post("/api/requests", body)?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success("Request submitted.");
    if let Some(id) = data.get("id").and_then(|v| v.as_str()) {
        println!("  ID: {}", id);
    }
    Ok(())
}

fn cmd_requests_delete(args: RequestDeleteArgs, ctx: &Context) -> Result<(), String> {
    if !args.yes {
        println!("About to delete request: {}", args.id.yellow());
        if !confirm("Are you sure?") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let data = ctx.auth_delete(&format!("/api/requests/{}", args.id))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success(&format!("Request {} deleted.", args.id));
    Ok(())
}

fn cmd_requests_status(args: RequestStatusArgs, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_patch(
        &format!("/api/requests/{}", args.id),
        json!({"status": args.status}),
    )?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success(&format!(
        "Request {} status set to {}.",
        args.id, args.status
    ));
    Ok(())
}

fn cmd_requests_fulfill(request_id: &str, snippet_id: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_put(
        &format!("/api/requests/{}/fulfillment", request_id),
        json!({"snippetId": snippet_id}),
    )?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success(&format!(
        "Request {} fulfilled with snippet {}.",
        request_id, snippet_id
    ));
    Ok(())
}

fn cmd_whoami(ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_get("/api/me")?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    let id = str_val(&data, "id");
    let created = uuid7_created(&id);
    print_detail(&[
        ("Username", str_val(&data, "username")),
        ("ID", id),
        ("Email", str_val(&data, "email")),
        ("Bio", str_val(&data, "description")),
        ("Role", str_val(&data, "role")),
        ("Created", created),
    ]);
    Ok(())
}

fn cmd_sessions(args: SessionsArgs, ctx: &Context) -> Result<(), String> {
    match args.action {
        None => {
            let data = ctx.auth_get("/api/me/sessions")?;
            if ctx.json_output {
                print_json(&data);
                return Ok(());
            }
            let sessions = data.as_array().cloned().unwrap_or_default();
            let rows: Vec<Vec<String>> = sessions
                .iter()
                .map(|s| {
                    vec![
                        str_val(s, "id"),
                        str_val(s, "userAgent"),
                        str_val(s, "ip"),
                        str_val(s, "createdAt"),
                    ]
                })
                .collect();
            print_table(&["ID", "USER AGENT", "IP", "CREATED"], &rows);
            Ok(())
        }
        Some(SessionsAction::Disconnect { yes }) => {
            if !yes {
                println!("{}", "About to disconnect all active sessions.".yellow());
                if !confirm("Are you sure?") {
                    println!("Aborted.");
                    return Ok(());
                }
            }
            let data = ctx.auth_delete("/api/me/sessions/others")?;
            if ctx.json_output {
                print_json(&data);
                return Ok(());
            }
            print_success("All sessions disconnected.");
            Ok(())
        }
    }
}

fn cmd_passkeys(args: PasskeysArgs, ctx: &Context) -> Result<(), String> {
    match args.action {
        None => {
            let data = ctx.auth_get("/api/auth/passkeys/list")?;
            if ctx.json_output {
                print_json(&data);
                return Ok(());
            }
            let keys = data.as_array().cloned().unwrap_or_default();
            let rows: Vec<Vec<String>> = keys
                .iter()
                .map(|k| {
                    vec![
                        str_val(k, "id"),
                        str_val(k, "label"),
                        str_val(k, "createdAt"),
                    ]
                })
                .collect();
            if rows.is_empty() {
                println!("No passkeys registered.");
            } else {
                print_table(&["ID", "LABEL", "CREATED"], &rows);
            }
            Ok(())
        }
        Some(PasskeysAction::Rename {
            credential_id,
            label,
        }) => {
            let data = ctx.auth_post(
                "/api/auth/passkeys/rename",
                json!({"credentialId": credential_id, "label": label}),
            )?;
            if ctx.json_output {
                print_json(&data);
                return Ok(());
            }
            print_success("Passkey renamed.");
            Ok(())
        }
        Some(PasskeysAction::Delete { credential_id, yes }) => {
            if !yes {
                println!("About to delete passkey: {}", credential_id.yellow());
                if !confirm("Are you sure?") {
                    println!("Aborted.");
                    return Ok(());
                }
            }
            let data = ctx.auth_post(
                "/api/auth/passkeys/delete",
                json!({"credentialId": credential_id}),
            )?;
            if ctx.json_output {
                print_json(&data);
                return Ok(());
            }
            print_success("Passkey deleted.");
            Ok(())
        }
    }
}

fn cmd_username(username: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_patch("/api/me/profile/username", json!({"username": username}))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success(&format!("Username updated to '{}'.", username));
    Ok(())
}

fn cmd_bio(bio: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_patch("/api/me/profile/description", json!({"description": bio}))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success("Bio updated.");
    Ok(())
}

fn cmd_privacy(args: PrivacyArgs, ctx: &Context) -> Result<(), String> {
    let mut body = serde_json::Map::new();
    if let Some(v) = args.visibility {
        body.insert("visibility".to_string(), json!(v));
    }
    if let Some(v) = args.default_list_visibility {
        body.insert("defaultListVisibility".to_string(), json!(v));
    }

    let mut show = serde_json::Map::new();
    if let Some(v) = args.show_bio {
        show.insert("bio".to_string(), json!(v));
    }
    if let Some(v) = args.show_snippets {
        show.insert("snippets".to_string(), json!(v));
    }
    if let Some(v) = args.show_lists {
        show.insert("lists".to_string(), json!(v));
    }
    if let Some(v) = args.show_comments {
        show.insert("comments".to_string(), json!(v));
    }
    if !show.is_empty() {
        body.insert("show".to_string(), Value::Object(show));
    }

    if body.is_empty() {
        return Err("No privacy settings specified.".to_string());
    }

    let data = ctx.auth_patch("/api/me/profile/privacy", Value::Object(body))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success("Privacy settings updated.");
    Ok(())
}

fn cmd_unlink(provider: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_delete(&format!("/api/me/providers/{}", provider))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success(&format!("Provider '{}' unlinked.", provider));
    Ok(())
}

fn cmd_delete_account(ctx: &Context) -> Result<(), String> {
    println!(
        "{}",
        "WARNING: This will permanently delete your account and all your data."
            .red()
            .bold()
    );
    println!();
    print!("Type {} to confirm: ", "DELETE MY ACCOUNT".red().bold());
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap_or(0);
    if input.trim() != "DELETE MY ACCOUNT" {
        println!("Aborted.");
        return Ok(());
    }

    let data = ctx.auth_delete("/api/me")?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success("Account deleted.");
    Ok(())
}

fn cmd_health(ctx: &Context) -> Result<(), String> {
    let data = ctx.get("/api/health")?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    let status = data
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    if status == "ok" || status == "healthy" {
        println!("  Status  {}", status.green().bold());
    } else {
        println!("  Status  {}", status.red().bold());
    }

    if let Some(obj) = data.as_object() {
        for (k, v) in obj {
            if k != "status" {
                let val = v.as_str().unwrap_or(&v.to_string()).to_string();
                println!("  {}  {}", k, val);
            }
        }
    }
    Ok(())
}

fn cmd_report(kind: &str, id: &str, reason: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_post(
        "/api/reports",
        json!({"targetType": kind, "targetId": id, "reason": reason}),
    )?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success("Report submitted. Thank you.");
    Ok(())
}

fn cmd_token(token: &str) -> Result<(), String> {
    persist_token(token)
}

#[cfg(unix)]
fn persist_token(token: &str) -> Result<(), String> {
    let home = std::env::var("HOME")
        .map_err(|_| "$HOME is not set — cannot locate shell rc file".to_string())?;
    let home = std::path::Path::new(&home);

    let shell = std::env::var("SHELL").unwrap_or_default();
    let shell_name = std::path::Path::new(&shell)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("sh")
        .to_string();

    let (rc_path, export_line) = match shell_name.as_str() {
        "zsh" => (
            home.join(".zshrc"),
            format!("export MICROCODES_API_TOKEN={}", token),
        ),
        "fish" => (
            home.join(".config/fish/config.fish"),
            format!("set -Ux MICROCODES_API_TOKEN {}", token),
        ),
        _ => (
            home.join(".bashrc"),
            format!("export MICROCODES_API_TOKEN={}", token),
        ),
    };

    if let Some(parent) = rc_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create {}: {}", parent.display(), e))?;
    }

    let existing = std::fs::read_to_string(&rc_path).unwrap_or_default();

    let has_existing = existing.lines().any(|l| {
        let t = l.trim();
        !t.starts_with('#') && t.contains("MICROCODES_API_TOKEN")
    });

    let new_content = if has_existing {
        let mut replaced = false;
        let lines: Vec<&str> = existing
            .lines()
            .filter_map(|l| {
                let t = l.trim();
                if !t.starts_with('#') && t.contains("MICROCODES_API_TOKEN") {
                    if !replaced {
                        replaced = true;
                        Some(export_line.as_str())
                    } else {
                        None
                    }
                } else {
                    Some(l)
                }
            })
            .collect();
        let mut out = lines.join("\n");
        if existing.ends_with('\n') {
            out.push('\n');
        }
        out
    } else {
        let mut out = existing.clone();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&export_line);
        out.push('\n');
        out
    };

    std::fs::write(&rc_path, &new_content)
        .map_err(|e| format!("Failed to write {}: {}", rc_path.display(), e))?;

    print_success(&format!("Token written to {}.", rc_path.display()));
    println!("  Run:  source {}", rc_path.display());
    println!("  Or open a new terminal session.");
    Ok(())
}

#[cfg(windows)]
fn persist_token(token: &str) -> Result<(), String> {
    // Escape single quotes for PowerShell string literal
    let escaped = token.replace('\'', "''");
    let ps_cmd = format!(
        "[System.Environment]::SetEnvironmentVariable('MICROCODES_API_TOKEN', '{}', 'User')",
        escaped
    );

    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_cmd])
        .status()
        .map_err(|e| format!("Failed to run PowerShell: {}", e))?;

    if !status.success() {
        return Err("PowerShell command failed — token was not saved.".to_string());
    }

    print_success("Token saved to your user environment variables.");
    println!("  Restart your terminal for the change to take effect.");
    Ok(())
}

fn parse_version(v: &str) -> (u32, u32, u32) {
    let mut parts = v.split('.').filter_map(|p| p.parse::<u32>().ok());
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

fn platform_artifact() -> Result<&'static str, String> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return Ok("microcodes-linux-x86_64");

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return Ok("microcodes-linux-aarch64");

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return Ok("microcodes-macos-x86_64");

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return Ok("microcodes-macos-aarch64");

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return Ok("microcodes-windows-x86_64.exe");

    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    return Err("Self-update is not supported on this platform.".to_string());
}

fn cmd_update(http: &HttpClient) -> Result<(), String> {
    let current = env!("CARGO_PKG_VERSION");

    println!("Current version:  {}", current);
    print!("Checking for updates... ");
    io::stdout().flush().ok();

    let resp = http
        .get("https://api.github.com/repos/KohanMathers/microcodes-cli/releases/latest")
        .send()
        .map_err(|e| format!("Failed to fetch release info: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("GitHub API returned HTTP {}", resp.status()));
    }

    let release: Value = resp
        .json()
        .map_err(|e| format!("Failed to parse release JSON: {}", e))?;

    let tag = release
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or("No tag_name in release response")?;

    let latest = tag.trim_start_matches('v');
    println!("Latest version:   {}", latest);

    if parse_version(latest) <= parse_version(current) {
        println!("{}", "Already up to date.".green());
        return Ok(());
    }

    let artifact_name = platform_artifact()?;

    let assets = release
        .get("assets")
        .and_then(|v| v.as_array())
        .ok_or("No assets in release response")?;

    let download_url = assets
        .iter()
        .find(|a| a.get("name").and_then(|v| v.as_str()) == Some(artifact_name))
        .and_then(|a| a.get("browser_download_url").and_then(|v| v.as_str()))
        .ok_or_else(|| format!("Asset '{}' not found in release", artifact_name))?
        .to_string();

    println!("Downloading {}...", artifact_name);

    let dl = http
        .get(&download_url)
        .send()
        .map_err(|e| format!("Download failed: {}", e))?;

    if !dl.status().is_success() {
        return Err(format!("Download returned HTTP {}", dl.status()));
    }

    let bytes = dl
        .bytes()
        .map_err(|e| format!("Failed to read downloaded binary: {}", e))?;

    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Cannot determine current executable path: {}", e))?;

    let tmp_path = exe_path.with_extension("tmp");

    {
        let f = std::fs::File::create(&tmp_path);

        #[cfg(unix)]
        let f = match f {
            Err(ref e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                eprintln!("Permission denied — retrying with sudo...");
                let exe = std::env::current_exe()
                    .map_err(|e| format!("Cannot determine executable path: {}", e))?;
                let status = std::process::Command::new("sudo")
                    .arg(exe)
                    .arg("update")
                    .status()
                    .map_err(|e| format!("Failed to run sudo: {}", e))?;
                process::exit(status.code().unwrap_or(1));
            }
            other => other,
        };

        let mut f = f.map_err(|e| format!("Cannot write to {}: {}", tmp_path.display(), e))?;
        f.write_all(&bytes)
            .map_err(|e| format!("Failed to write binary: {}", e))?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set executable permissions: {}", e))?;
    }

    // Atomic replacement
    #[cfg(unix)]
    std::fs::rename(&tmp_path, &exe_path)
        .map_err(|e| format!("Failed to replace binary: {}", e))?;

    #[cfg(windows)]
    {
        let old_path = exe_path.with_extension("old");
        let _ = std::fs::remove_file(&old_path);
        std::fs::rename(&exe_path, &old_path)
            .map_err(|e| format!("Failed to move current binary aside: {}", e))?;
        if let Err(e) = std::fs::rename(&tmp_path, &exe_path) {
            let _ = std::fs::rename(&old_path, &exe_path);
            return Err(format!("Failed to place new binary: {}", e));
        }
        let _ = std::fs::remove_file(&old_path);
    }

    print_success(&format!("Updated to v{}.", latest));
    Ok(())
}

fn cmd_feedback(message: &str, ctx: &Context) -> Result<(), String> {
    let data = ctx.auth_post("/api/feedback", json!({"message": message}))?;

    if ctx.json_output {
        print_json(&data);
        return Ok(());
    }

    print_success("Feedback sent. Thank you!");
    Ok(())
}

fn main() {
    let cli = Cli::parse();

    let use_color = !cli.plain && io::stdout().is_terminal();
    colored::control::set_override(use_color);

    let base_url =
        std::env::var("MICROCODES_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
    let token = std::env::var("MICROCODES_API_TOKEN").ok();

    let ctx = Context {
        base_url,
        token,
        json_output: cli.json,
        http: HttpClient::builder()
            .user_agent(concat!("microcodes-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("Failed to build HTTP client"),
    };

    let Some(command) = cli.command else {
        if let Err(e) = tui::run_tui(ctx.base_url, ctx.token) {
            eprintln!("{} {}", "Error:".red().bold(), e);
            process::exit(1);
        }
        return;
    };

    let result = match command {
        Commands::Search(a) => cmd_search(a, &ctx),
        Commands::Get { id } => cmd_get(&id, &ctx),
        Commands::Ids { ids } => cmd_ids(&ids, &ctx),
        Commands::MySnippets => cmd_my_snippets(&ctx),
        Commands::Submit(a) => cmd_submit(a, &ctx),
        Commands::Delete(a) => cmd_delete(a, &ctx),
        Commands::Patch(a) => cmd_patch(a, &ctx),
        Commands::Versions { id } => cmd_versions(&id, &ctx),
        Commands::Diff(a) => cmd_diff(a, &ctx),
        Commands::Metrics { id } => cmd_metrics(&id, &ctx),
        Commands::Vote { id, direction } => cmd_vote(&id, direction, &ctx),
        Commands::Bookmark { id, remove } => cmd_bookmark(&id, !remove, &ctx),
        Commands::Schema => cmd_schema(&ctx),
        Commands::Markdown(a) => cmd_markdown(a, &ctx),
        Commands::Comment(a) => cmd_comment(a, &ctx),
        Commands::Lists(a) => cmd_lists(a, &ctx),
        Commands::Requests(a) => cmd_requests(a, &ctx),
        Commands::Whoami => cmd_whoami(&ctx),
        Commands::Sessions(a) => cmd_sessions(a, &ctx),
        Commands::Passkeys(a) => cmd_passkeys(a, &ctx),
        Commands::Username { username } => cmd_username(&username, &ctx),
        Commands::Bio { bio } => cmd_bio(&bio, &ctx),
        Commands::Privacy(a) => cmd_privacy(a, &ctx),
        Commands::Unlink { provider } => cmd_unlink(&provider, &ctx),
        Commands::DeleteAccount => cmd_delete_account(&ctx),
        Commands::Health => cmd_health(&ctx),
        Commands::Report { kind, id, reason } => cmd_report(&kind, &id, &reason, &ctx),
        Commands::Feedback { message } => cmd_feedback(&message, &ctx),
        Commands::Update => cmd_update(&ctx.http),
        Commands::Token { token } => cmd_token(&token),
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(1);
    }
}
