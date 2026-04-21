# microcodes CLI

A fast, single-binary command-line interface for the [Microcodes](https://micro.codes) API. Interact with snippets, lists, requests, comments, and your account entirely from the terminal.

Available as both `microcodes` and the shorter alias `mcodes`.

---

## Installation

### Linux / macOS

You need [Rust](https://rustup.rs) installed.

```bash
git clone https://github.com/kohanmathers/microcodes-cli
cd microcodes-cli
bash install.sh
```

This will:
1. Build a release binary with `cargo build --release`
2. Copy it to `/usr/local/bin/microcodes`
3. Create a symlink `/usr/local/bin/mcodes -> microcodes`

### Windows

You need [Rust](https://rustup.rs) installed.

```powershell
git clone https://github.com/kohanmathers/microcodes-cli
cd microcodes-cli
powershell -ExecutionPolicy Bypass -File install.ps1
```

This will:
1. Build a release binary
2. Copy it to `%APPDATA%\microcodes\microcodes.exe`
3. Create `mcodes.bat` and `microcodes.bat` wrappers in the same directory
4. Add the directory to your user `PATH` via the registry

### Manual build

```bash
cargo build --release
# Binary is at ./target/release/microcodes
```

---

## Authentication

The easiest way to log in is with the device flow command:

```bash
mcodes login
```

This prints a URL and a short confirmation code. Open the URL in your browser, approve the request, and the CLI saves your API key automatically. No copying tokens by hand.

If you prefer to set a token manually, use the `token` command:

```bash
mcodes token your_key_here
```

Both commands write the key to your shell profile (`~/.bashrc`, `~/.zshrc`, etc.) and tell you to source it or open a new terminal. On Windows the key is written to your user environment variables.

You can also set the variable directly without persisting it:

```bash
export MICROCODES_API_TOKEN=your_key_here
```

If you run an authenticated command without a token set, you'll see:

```
Error: MICROCODES_API_TOKEN is not set.
Export it with: export MICROCODES_API_TOKEN=your_key_here
```

---

## Global flags

These flags work with every command:

| Flag               | Description                                 |
| ------------------ | ------------------------------------------- |
| `--json`           | Output raw JSON instead of formatted tables |
| `--plain`          | Suppress colour and formatting              |
| `--help` / `-h`    | Show help for any command                   |
| `--version` / `-V` | Show version                                |

Colour is also disabled automatically when stdout is not a TTY (e.g. when piping output).

---

## Environment variables

| Variable               | Default               | Description                                               |
| ---------------------- | --------------------- | --------------------------------------------------------- |
| `MICROCODES_API_TOKEN` | —                     | API authentication token (required for auth'd commands)   |
| `MICROCODES_BASE_URL`  | `https://micro.codes` | Override the API base URL (useful for self-hosted or dev) |

---

## Command reference

### Search & discovery

#### `mcodes search <query>`

Search for snippets.

```bash
mcodes search "nginx reverse proxy"
mcodes search "docker" --tags docker,nginx --languages yaml
mcodes search "auth" --submitter me --sort newest
mcodes search "redis" --generated only --page 2
```

Flags:
- `--tags <tag1,tag2>` — filter by tags (prefix with `!` to exclude, e.g. `!ai`)
- `--languages <lang1,lang2>` — filter by language
- `--submitter <username>` — filter by submitter (`me` for your own snippets)
- `--generated <include|exclude|only>` — filter AI-generated snippets
- `--sort <relevance|oldest|newest|upvotes>` — sort order (default: relevance)
- `--page <n>` — page number

#### `mcodes get <id>`

Fetch a single snippet by ID and print its full detail.

```bash
mcodes get 0195f2ad-cfd6-7f0a-8b76-c4643b92d4db
```

#### `mcodes ids <id1,id2,...>`

Fetch multiple snippets by comma-separated IDs.

```bash
mcodes ids "abc123,def456,ghi789"
```

#### `mcodes my-snippets`

List all snippets you have submitted. Requires authentication.

```bash
mcodes my-snippets
mcodes my-snippets --json
```

---

### Snippets

#### `mcodes submit`

Submit a new snippet. Reads JSON from stdin or a file. The schema is validated against `GET /api/schema` before submitting. Requires authentication.

```bash
mcodes submit --file my-snippet.json
cat snippet.json | mcodes submit
```

#### `mcodes delete <id>`

Delete a snippet. Prompts for confirmation unless `--yes` / `-y` is passed. Requires authentication.

```bash
mcodes delete 0195f2ad-cfd6-7f0a-8b76-c4643b92d4db
mcodes delete 0195f2ad-cfd6-7f0a-8b76-c4643b92d4db --yes
```

#### `mcodes patch <id> [flags]`

Update fields on a snippet. Requires authentication.

```bash
mcodes patch abc123 --title "Better title"
mcodes patch abc123 --description "Updated description"
mcodes patch abc123 --field "language=TypeScript" --field "tags=ts,node"
```

Flags:
- `--title <text>`
- `--description <text>`
- `--field key=value` (repeatable, for arbitrary fields)

#### `mcodes versions <id>`

List the version history of a snippet.

```bash
mcodes versions abc123
```

#### `mcodes diff <id> --from <n> --to <n>`

Show a diff between two versions.

```bash
mcodes diff abc123 --from 1 --to 3
```

#### `mcodes metrics <id>`

Show upvote, downvote, and bookmark counts.

```bash
mcodes metrics abc123
```

#### `mcodes vote <id> <up|down>`

Vote on a snippet. Requires authentication.

```bash
mcodes vote abc123 up
mcodes vote abc123 down
```

#### `mcodes bookmark <id>`

Toggle a bookmark on a snippet. Requires authentication.

```bash
mcodes bookmark abc123
```

#### `mcodes schema`

Print the JSON schema for snippet submission.

```bash
mcodes schema
mcodes schema | jq '.required'
```

#### `mcodes markdown`

Render markdown to HTML. Reads from stdin or a file.

```bash
mcodes markdown --file README.md
echo "# Hello" | mcodes markdown
```

---

### Comments

#### `mcodes comment add <snippet-id> "<text>"`

Post a comment on a snippet. Requires authentication.

```bash
mcodes comment add abc123 "Great snippet!"
mcodes comment add abc123 "Agreed" --reply-to comment456
```

Flags:
- `--reply-to <comment-id>` — reply to an existing comment

#### `mcodes comment vote <comment-id> <up|down>`

Vote on a comment. Requires authentication.

```bash
mcodes comment vote comment456 up
```

#### `mcodes comment edit <comment-id> "<new text>"`

Edit your comment. Requires authentication.

```bash
mcodes comment edit comment456 "Updated text"
```

#### `mcodes comment delete <comment-id>`

Delete a comment. Prompts for confirmation unless `--yes` / `-y` is passed. Requires authentication.

```bash
mcodes comment delete comment456
mcodes comment delete comment456 --yes
```

---

### Lists

#### `mcodes lists`

List all your lists. Requires authentication.

```bash
mcodes lists
```

#### `mcodes lists create "<title>"`

Create a new list. Requires authentication.

```bash
mcodes lists create "My Favourites"
mcodes lists create "DevOps Tools" --description "Useful ops snippets" --unlisted
```

Flags:
- `--description "<text>"`
- `--unlisted` — hide from public

#### `mcodes lists get <id>`

Get a list by ID, including its snippets.

```bash
mcodes lists get list123
```

#### `mcodes lists user <user-id>`

Get public lists for a user.

```bash
mcodes lists user user456
```

#### `mcodes lists update <id> [flags]`

Update a list. Requires authentication.

```bash
mcodes lists update list123 --title "New Title"
mcodes lists update list123 --unlisted true
```

Flags: `--title`, `--description`, `--unlisted <true|false>`

#### `mcodes lists delete <id>`

Delete a list. Requires authentication.

```bash
mcodes lists delete list123
mcodes lists delete list123 --yes
```

#### `mcodes lists add <list-id> <snippet-id>`

Add a snippet to a list. Requires authentication.

```bash
mcodes lists add list123 snippet456
```

#### `mcodes lists remove <list-id> <snippet-id>`

Remove a snippet from a list. Requires authentication.

```bash
mcodes lists remove list123 snippet456
```

---

### Requests

#### `mcodes requests`

List snippet requests.

```bash
mcodes requests
mcodes requests --status open
mcodes requests --user user123 --limit 20 --offset 0
```

Flags:
- `--status <open|fulfilled|closed|rejected>`
- `--user <user-id>`
- `--limit <n>`
- `--offset <n>`

#### `mcodes requests get <id>`

Get a request by ID.

```bash
mcodes requests get req123
```

#### `mcodes requests submit "<title>" "<description>"`

Submit a new snippet request. Requires authentication.

```bash
mcodes requests submit "Nginx auth proxy" "A snippet for authenticating requests via nginx"
mcodes requests submit "Redis cache" "Simple Redis cache setup" --tags redis,caching
```

#### `mcodes requests delete <id>`

Delete a request. Requires authentication.

```bash
mcodes requests delete req123
mcodes requests delete req123 --yes
```

#### `mcodes requests status <id> <status>`

Update request status. Requires authentication.

```bash
mcodes requests status req123 fulfilled
mcodes requests status req123 closed
```

Status values: `open`, `fulfilled`, `closed`, `rejected`

#### `mcodes requests fulfill <request-id> <snippet-id>`

Fulfill a request with a snippet. Requires authentication.

```bash
mcodes requests fulfill req123 snippet456
```

---

### Auth & account

#### `mcodes whoami`

Show your current user profile. Requires authentication.

```bash
mcodes whoami
```

#### `mcodes sessions`

List your active sessions. Requires authentication.

```bash
mcodes sessions
```

#### `mcodes sessions disconnect`

Disconnect all active sessions. Requires authentication.

```bash
mcodes sessions disconnect
mcodes sessions disconnect --yes
```

#### `mcodes username "<new-username>"`

Change your username. Requires authentication.

```bash
mcodes username "mynewname"
```

#### `mcodes bio "<new-bio>"`

Update your profile bio. Requires authentication.

```bash
mcodes bio "Senior SRE. I write things that run in containers."
```

#### `mcodes privacy [flags]`

Update privacy settings. Requires authentication.

```bash
mcodes privacy --visibility public
mcodes privacy --show-bio true --show-snippets false
mcodes privacy --default-list-visibility unlisted
```

Flags:
- `--visibility <public|unlisted|private>`
- `--show-bio <true|false>`
- `--show-snippets <true|false>`
- `--show-lists <true|false>`
- `--show-comments <true|false>`
- `--default-list-visibility <public|unlisted>`

#### `mcodes unlink <github|google|gitlab>`

Unlink an OAuth provider from your account. Requires authentication.

```bash
mcodes unlink github
```

#### `mcodes delete-account`

Permanently delete your account. You will be prompted to type `DELETE MY ACCOUNT` to confirm — there is no `--yes` shortcut. Requires authentication.

```bash
mcodes delete-account
```

---

### Other

#### `mcodes health`

Check service status.

```bash
mcodes health
```

#### `mcodes report <snippet|user|comment> <id> "<reason>"`

Report content. Requires authentication.

```bash
mcodes report snippet abc123 "Spam / low quality"
mcodes report user user456 "Abusive behaviour"
mcodes report comment comment789 "Harassment"
```

#### `mcodes feedback "<message>"`

Send feedback to the Microcodes team. Requires authentication.

```bash
mcodes feedback "Would love to see a dark mode API endpoint"
```

#### `mcodes user <username>`

Get info about a user

```bash
mcodes user KohanMathers
```

---

## Output formats

**Default** — formatted tables for lists, labelled detail views for single items:

```
  ID                                    TITLE                          LANGUAGE   UPVOTES
  0195f2ad-cfd6-7f0a-8b76-c4643b92d4db  Docker Compose Nginx Proxy     YAML       42
```

```
  Title       Docker Compose Nginx Proxy
  ID          0195f2ad-cfd6-7f0a-8b76-c4643b92d4db
  Language    YAML
  Tags        docker, nginx
  Upvotes     42
  Bookmarks   7
```

**`--json`** — raw API response, pretty-printed. Pipe to `jq` for further processing:

```bash
mcodes search "redis" --json | jq '.[].id'
```

**`--plain`** — same structured output but without ANSI colour codes. Useful for scripts or editors that don't handle colour.

---

## Error codes and messages

| Situation        | Message                                                   |
| ---------------- | --------------------------------------------------------- |
| Token not set    | `Error: MICROCODES_API_TOKEN is not set.`                 |
| 401 Unauthorized | `Error: Not authenticated. Set MICROCODES_API_TOKEN.`     |
| 403 Forbidden    | `Error: Forbidden. You don't have permission to do that.` |
| 404 Not Found    | `Error: Not found.`                                       |
| Other 4xx/5xx    | `Error: HTTP <code>: <message from API>`                  |
| Network failure  | `Error: Connection error (https://...): <details>`        |

All errors are printed to stderr. The process exits with code `1` on any error.