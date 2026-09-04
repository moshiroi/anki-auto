# anki-auto

`anki-auto` imports Japanese vocabulary JSON into Anki through AnkiConnect. It validates input, creates the deck and `jp-vocab` note type when needed, avoids duplicate words in the target deck, tags source provenance, and can sync after importing.

## Prerequisites

- macOS with Anki installed and the AnkiConnect add-on enabled
- AnkiWeb configured in Anki if you want sync
- Nix, then enter the development shell with `nix develop` (this provides Rust and the `aa` alias)
- For the Codex YouTube workflow: an awake laptop, internet access, and Codex opened in this project

Anki does not have to be open initially: on macOS the CLI attempts to launch it and waits for AnkiConnect. This is laptop-local automation and does not run while the laptop is asleep.

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

If notes import successfully but sync fails, the CLI reports the imported counts, exits with an error, and directs you to retry with `aa sync`. The already-imported notes are not rolled back.

## Development safety

Tests do not require a live Anki instance and must not import into or sync a real collection. Use `--dry-run` for manual CLI checks. If an explicitly authorized live integration check is ever necessary, target a disposable deck explicitly:

```sh
aa import test-cards.json --deck anki-auto-test --source "local integration test"
```

Never rely on the default `Japanese` deck for development or testing. The Codex production workflow imports only after an actual user request to make cards.
