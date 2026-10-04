# anki-auto

`anki-auto` imports JSON cards into your Anki decks through AnkiConnect. Use any existing note type with `--model`, or use the built-in Japanese vocabulary format. It validates input, skips duplicates, tags source provenance, and optionally syncs. No AI API key or hosted service is needed.

## Get your first card into Anki

### 1. Set up Anki

Install [Anki Desktop](https://apps.ankiweb.net/), then open it. In **Tools → Add-ons → Get Add-ons**, enter `2055492159` to install [AnkiConnect](https://ankiweb.net/shared/info/2055492159). Restart Anki after installing the add-on and leave it open.

AnkiWeb is optional: you can import and review cards locally without an account. Configure AnkiWeb later if you want cards on other devices.

### 2. Install the tool

No Rust, Nix, or administrator access is needed for the [downloadable binaries](https://github.com/moshiroi/anki-auto/releases/latest).

**macOS / Linux** — paste this into Terminal (bash or zsh):

```sh
curl -fsSL https://github.com/moshiroi/anki-auto/releases/latest/download/install.sh | sh
export PATH="$HOME/.local/share/anki-auto:$PATH"
anki-auto --version
```

**Windows x64** — paste this into PowerShell:

```powershell
Invoke-RestMethod https://github.com/moshiroi/anki-auto/releases/latest/download/install.ps1 | Invoke-Expression
anki-auto --version
```

The installer detects your platform, verifies the archive checksum, and installs into your user folder. It updates PATH for future terminals (bash/zsh on macOS/Linux; user PATH on Windows). The Unix `export` above makes the command available in your current terminal too. Run the same installer again to update.

| Platform | Archive | Installer location |
| --- | --- | --- |
| macOS 13+, Apple Silicon | `anki-auto-aarch64-apple-darwin.tar.gz` | `~/.local/share/anki-auto` |
| macOS 13+, Intel | `anki-auto-x86_64-apple-darwin.tar.gz` | `~/.local/share/anki-auto` |
| Linux x64 | `anki-auto-x86_64-unknown-linux-musl.tar.gz` | `~/.local/share/anki-auto` |
| Linux ARM64 | `anki-auto-aarch64-unknown-linux-musl.tar.gz` | `~/.local/share/anki-auto` |
| Windows x64 | `anki-auto-x86_64-pc-windows-msvc.zip` | `%LOCALAPPDATA%\anki-auto` |

Prefer a manual download? Extract the matching archive from [Releases](https://github.com/moshiroi/anki-auto/releases/latest). It contains the binary, `basic.json`, the MIT license, and instructions. You can run the binary directly from that folder (`./anki-auto --help` or `.\anki-auto.exe --help`) without changing PATH.

macOS builds are not Apple-notarized. If macOS blocks a manually downloaded binary, use its **Privacy & Security → Open Anyway** option for this download. [Apple's instructions](https://support.apple.com/en-us/102445) explain the process. Do not disable Gatekeeper globally.

<details>
<summary>Build from source instead</summary>

Install [Git](https://git-scm.com/downloads) and [Rust and Cargo](https://www.rust-lang.org/tools/install) for your operating system. Reopen your terminal afterward, then check `cargo --version`.

```sh
git clone https://github.com/moshiroi/anki-auto.git
cd anki-auto
cargo install --path . --locked
anki-auto --help
```

If `anki-auto` is not found, ensure Cargo's bin directory is on PATH: `~/.cargo/bin` on macOS/Linux or `%USERPROFILE%\.cargo\bin` on Windows. Nix users can instead run `nix develop`; this provides Rust and an `aa` shortcut. Use `aa` wherever the examples say `anki-auto`.

</details>

### 3. Check the connection

Keep Anki open and run:

```sh
anki-auto ping
```

You should see `connected to AnkiConnect` followed by its version. If it fails, check that AnkiConnect is enabled under **Tools → Add-ons**, restart Anki, and try again. `--dry-run` only checks input; it does not check this connection.

### 4. Import the included example

The installer includes a sample—no need to write your own JSON yet.

**macOS / Linux:**

```sh
anki-auto import "$HOME/.local/share/anki-auto/basic.json" --deck Geography --model Basic --dry-run
anki-auto import "$HOME/.local/share/anki-auto/basic.json" --deck Geography --model Basic
```

**Windows:**

```powershell
anki-auto import "$env:LOCALAPPDATA\anki-auto\basic.json" --deck Geography --model Basic --dry-run
anki-auto import "$env:LOCALAPPDATA\anki-auto\basic.json" --deck Geography --model Basic
```

For manual downloads, run the extracted binary with `basic.json` from its folder. Source installs include the sample at `tests/fixtures/basic.json` in the repository.

Expect `parsed 1 entry` from the dry run and `1 added` from the import. Open the **Geography** deck in Anki to review your first card. Repeating the import reports `0 added` and `1 skipped (duplicates)`.

If your note type is named differently, find its name under **Tools → Manage Note Types** and replace `Basic` with that exact name. The included example needs `Front` and `Back` fields.

### 5. Make your own cards

Create `cards.json` with keys matching your note type's field names exactly (including capitalization). For standard `Basic`:

```json
[
  { "Front": "What is the capital of France?", "Back": "Paris" }
]
```

```sh
anki-auto import cards.json --deck Geography --model Basic --dry-run
anki-auto import cards.json --deck Geography --model Basic --source "Geography notes"
```

When AnkiWeb is configured, sync separately with `anki-auto sync` or add `--sync` to an import. A sync error does not undo a successful local import; retry with `anki-auto sync`.

The deck is created if needed. The note type must already exist in Anki; its templates and styling control how cards look. Custom input values are strings passed through unchanged, including Anki HTML, cloze markup, and existing media references. Media uploading is not included.

The first field **in the Anki note type's field order** must be present and non-empty; it is the duplicate key, regardless of JSON key order. Other fields may be omitted or empty. Duplicates are skipped within a batch and within the selected deck (including subdecks) for the same note type. Different note types can share the same first-field value.

`--dry-run` checks JSON structure and values without contacting Anki. During import, the CLI also checks field names and the required first field against the actual note type before creating the deck or adding notes. Unknown fields or missing note types fail with an error. A cloze note type additionally requires valid cloze markup for Anki to generate cards.

For ongoing file imports:

```sh
anki-auto watch --inbox inbox --deck Geography --model Basic --tags study
```

For an agent-driven workflow, tell your agent the target deck, note type, and exact fields, and ask it to produce JSON, dry-run it, then import. The YouTube/Japanese instructions in this repository are an example workflow; custom cards do not need Japanese vocabulary fields.

AnkiConnect defaults to `http://127.0.0.1:8765`; override it with `ANKI_CONNECT_URL`. On Linux or Windows, start Anki manually.

## Note type examples

Find your exact note type name under **Tools → Manage Note Types** in Anki; select it and click **Fields** to see its field names and order. Names may differ if you renamed them or use another interface language. These examples assume the standard English names described in the [Anki manual](https://docs.ankiweb.net/getting-started.html#note-types).

Save each JSON example to the filename shown in its command. Run the command with `--dry-run` first, then remove `--dry-run` to import. Add `--sync` only if AnkiWeb is configured. Counts reported by the CLI are **notes**, which can generate more than one review card.

The images below are browser-rendered previews of the example data, with the question on the left and revealed answer on the right. Standard examples follow [Anki's stock templates](https://github.com/ankitects/anki/blob/main/rslib/src/notetype/stock.rs); the custom template is illustrative. Your templates, theme, and device can change the appearance.

### Basic

One question-and-answer card per note:

```json
{ "Front": "What is the capital of France?", "Back": "Paris" }
```

![Basic note: question on the left, Paris revealed on the right](docs/images/basic.png)

```sh
anki-auto import basic.json --deck Geography --model Basic --dry-run
```

### Basic (and reversed card)

Two cards per note: French → English and English → French.

```json
{ "Front": "bonjour", "Back": "hello" }
```

![Reversed note: both French-to-English and English-to-French cards](docs/images/reversed.png)

```sh
anki-auto import reversed.json --deck French --model "Basic (and reversed card)" --dry-run
```

### Basic (optional reversed card)

Set `Add Reverse` to any non-empty string to generate the reverse card; omit it or leave it empty for just the forward card.

```json
{ "Front": "bonjour", "Back": "hello", "Add Reverse": "yes" }
```

![Optional reverse note: both directions generated when Add Reverse is yes](docs/images/optional-reversed.png)

```sh
anki-auto import optional-reversed.json --deck French --model "Basic (optional reversed card)" --dry-run
```

### Basic (type in the answer)

Uses the same fields as Basic, with a typed-answer prompt in Anki:

```json
{ "Front": "French for hello", "Back": "bonjour" }
```

![Typed-answer note: answer input and a correctly typed answer](docs/images/typed.png)

```sh
anki-auto import typed.json --deck French --model "Basic (type in the answer)" --dry-run
```

### Cloze

Use `{{c1::answer}}` inside `Text` for a blank. Different numbers (`c1`, `c2`, etc.) generate separate cards; `Back Extra` is optional. See [Anki's cloze documentation](https://docs.ankiweb.net/editing.html#cloze-deletion).

```json
{
  "Text": "Paris is the capital of {{c1::France}}.",
  "Back Extra": "Paris is on the Seine."
}
```

![Cloze note: France hidden in the question and revealed in the answer](docs/images/cloze.png)

```sh
anki-auto import cloze.json --deck Geography --model Cloze --dry-run
```

### Your own note type

For an existing note type named `French Vocabulary`, with `French` as its first field and additional `English` and `Example` fields:

```json
{
  "French": "bonjour",
  "English": "hello",
  "Example": "Bonjour, comment allez-vous ?"
}
```

![Illustrative French Vocabulary template showing translation and example](docs/images/custom.png)

```sh
anki-auto import french.json --deck French --model "French Vocabulary" --dry-run
```

Use your own field names and note type name. Templates stay in Anki; the importer passes field values through.

The built-in Japanese format is shown in the CLI section below. It uses lowercase vocabulary keys and automatically creates `jp-vocab`; omit `--model` for that format.

## Codex workflow

Ask Codex in this project, for example:

> Make Anki cards from this YouTube video: https://www.youtube.com/watch?v=VIDEO_ID

> Make Anki cards from these YouTube videos: https://youtu.be/VIDEO_1 and https://youtu.be/VIDEO_2. Focus on intermediate vocabulary.

> I didn't get enough passive Japanese immersion today. Find videos similar to sources I've used before and create 20 cards.

The repository instructions tell Codex to retrieve available Japanese captions, make only transcript-grounded cards, validate them with a dry run, import each source directly without clipboard copying, and sync. In discovery mode, Codex reads prior source tags, searches for similar captioned material, and treats the requested number as a total of newly added cards across all videos. It keeps looking after duplicate skips when supported vocabulary remains, but never fabricates or pads cards; if it cannot safely reach the target, it reports the achieved count and why.

Discovery intentionally stays agent-driven: Codex performs live search and qualitative similarity judgment. The Rust CLI does not contain a YouTube recommendation algorithm or require an AI/API key; it is responsible only for exposing provenance and safely validating, deduplicating, importing, and syncing cards.

## CLI

The input can be one card object or an array:

```json
{
  "word": "食べる",
  "reading": "たべる",
  "meaning": "to eat",
  "sentence": "昨日寿司を食べました。",
  "sentence_meaning": "I ate sushi yesterday."
}
```

![Built-in Japanese vocabulary note with reading, meaning, and highlighted sentence](docs/images/japanese.png)

`sentence_meaning` is optional. Required fields must be non-empty.

```sh
# Validate only; never contacts or launches Anki
aa import cards.json --dry-run --source "Video title https://youtu.be/VIDEO_ID"

# Import, tag provenance, and sync in one invocation
aa import cards.json --tags youtube --source "Video title https://youtu.be/VIDEO_ID" --sync

# Retry sync without re-importing
aa sync

# Read-only source history for discovery (most-used first)
aa sources
aa sources --deck Japanese --limit 10

# Existing stdin and watch workflows remain available
printf '%s\n' "$CARDS_JSON" | aa import --source "source name"
aa watch --inbox inbox --tags youtube
```

If Anki rejects individual notes, the CLI reports them as failures with their input entry numbers and exits with an error. Successfully added notes remain local. Fix the rejected entries and rerun the import; existing notes are skipped. Sync does not run after a partial failure. Watch mode moves partially failed batches to `failed/` so they can be fixed and retried.

If notes import successfully but sync fails, the CLI reports the imported counts, exits with an error, and directs you to retry with `aa sync`. The already-imported notes are not rolled back.

## Development safety

Tests do not require a live Anki instance and must not import into or sync a real collection. Use `--dry-run` for manual CLI checks. If an explicitly authorized live integration check is ever necessary, target a disposable deck explicitly:

```sh
aa import test-cards.json --deck anki-auto-test --source "local integration test"
```

Never rely on the default `Japanese` deck for development or testing. The Codex production workflow imports only after an actual user request to make cards.

## Contributing and license

Contributions are welcome through pull requests; see [CONTRIBUTING.md](CONTRIBUTING.md). Changes to `main` require owner review.

Licensed under [MIT](LICENSE).

## Release builds and privacy

Release binaries are built on GitHub-hosted runners, not a maintainer's computer. Release builds strip debug symbols, remap workspace and home-directory paths, and are scanned for embedded build/user paths before packaging. Archives include only the binary, sample JSON, license, and install instructions; Unix archive ownership is normalized. They do not include local Anki collections, credentials, or developer configuration.

Linux builds use static musl linking. Windows builds statically link the C runtime; macOS and Windows still use normal OS libraries. These checks remove personal build paths; generic compiler/source references and platform metadata can remain. SHA-256 checksums detect damaged downloads but are not code-signing certificates. See [the release checklist](docs/RELEASING.md) for the build and verification process.
