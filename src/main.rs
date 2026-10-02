mod anki;
mod cards;
mod vocab;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use crate::anki::AnkiClient;
use crate::cards::Card;

#[derive(Parser)]
#[command(name = "anki-auto", about = "Import JSON cards into Anki")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check that Anki is running and AnkiConnect is reachable
    Ping,
    /// Sync the local Anki collection with AnkiWeb
    Sync,
    /// List previously imported source tags from a deck, most-used first
    Sources {
        /// Deck whose source history should be inspected
        #[arg(long, default_value = "Japanese")]
        deck: String,
        /// Maximum number of sources to print
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Import card entries from a JSON file (or stdin if no file given)
    Import {
        /// Path to JSON file; omit to read from stdin
        path: Option<PathBuf>,
        /// Target deck (created if missing)
        #[arg(long, default_value = "Japanese")]
        deck: String,
        /// Existing Anki note type; input keys must match its field names
        #[arg(long)]
        model: Option<String>,
        /// Comma-separated tags for the notes
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Provenance for this import batch, such as a video title or URL
        #[arg(long)]
        source: Option<String>,
        /// Parse and validate only, without touching Anki
        #[arg(long)]
        dry_run: bool,
        /// Sync with AnkiWeb after a successful import
        #[arg(long)]
        sync: bool,
    },
    /// Watch a directory and import any JSON file that appears
    Watch {
        /// Directory to watch (created if missing)
        #[arg(long, default_value = "inbox")]
        inbox: PathBuf,
        /// Target deck (created if missing)
        #[arg(long, default_value = "Japanese")]
        deck: String,
        /// Existing Anki note type; input keys must match its field names
        #[arg(long)]
        model: Option<String>,
        /// Comma-separated tags for the notes
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Provenance tag applied to every watched batch
        #[arg(long)]
        source: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Ping => {
            let client = AnkiClient::default();
            println!("connected to AnkiConnect {}", client.version()?);
        }
        Command::Sync => {
            AnkiClient::default().sync()?;
            println!("sync complete");
        }
        Command::Sources { deck, limit } => {
            let sources = AnkiClient::default().sources(&deck)?;
            if sources.is_empty() {
                println!("no source provenance found in deck `{deck}`");
            } else {
                for source in sources.into_iter().take(limit) {
                    println!("{}\t{}", source.note_count, source.source);
                }
            }
        }
        Command::Import {
            path,
            deck,
            tags,
            model,
            source,
            dry_run,
            sync,
        } => {
            let entries = cards::load(path.as_deref(), model.is_some())?;
            ensure_non_empty(&entries)?;
            if dry_run {
                print_entries(&entries);
            } else {
                let client = AnkiClient::default();
                let counts = push(
                    &client,
                    &entries,
                    &deck,
                    &tags,
                    source.as_deref(),
                    model.as_deref(),
                )?;
                report(counts);
                if sync {
                    client.sync().context(
                        "notes were imported, but sync failed; retry with `anki-auto sync`",
                    )?;
                    println!("sync complete");
                }
            }
        }
        Command::Watch {
            inbox,
            deck,
            tags,
            model,
            source,
        } => watch(&inbox, &deck, &tags, source.as_deref(), model.as_deref())?,
    }
    Ok(())
}

fn ensure_non_empty(entries: &[Card]) -> Result<()> {
    if entries.is_empty() {
        bail!("no card entries found in input");
    }
    Ok(())
}

fn print_entries(entries: &[Card]) {
    println!(
        "parsed {} entr{}:",
        entries.len(),
        if entries.len() == 1 { "y" } else { "ies" }
    );
    for entry in entries {
        println!(
            "  {}",
            serde_json::to_string(entry).expect("string fields serialize")
        );
    }
}

fn push(
    client: &AnkiClient,
    entries: &[Card],
    deck: &str,
    tags: &[String],
    source: Option<&str>,
    model: Option<&str>,
) -> Result<(usize, usize)> {
    let model_name = model.unwrap_or(anki::MODEL_NAME);
    let fields = if model.is_some() {
        client.model_fields(model_name)?
    } else {
        client.ensure_model()?;
        client.model_fields(model_name)?
    };
    cards::validate_model(entries, &fields)?;
    client.ensure_deck(deck)?;
    client.add_notes(deck, model_name, &fields[0], tags, source, entries)
}

fn report((added, skipped): (usize, usize)) {
    println!("{added} added");
    if skipped > 0 {
        println!("{skipped} skipped (duplicates)");
    }
}

fn is_stale(path: &Path) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|mtime| {
            mtime
                .elapsed()
                .map(|age| age > Duration::from_secs(3))
                .unwrap_or(true)
        })
        .unwrap_or(true)
}

fn archive(file: &Path, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let dest = dir.join(file.file_name().context("file has no name")?);
    std::fs::rename(file, &dest).with_context(|| format!("failed to archive {}", file.display()))
}

fn watch(
    inbox: &Path,
    deck: &str,
    tags: &[String],
    source: Option<&str>,
    model: Option<&str>,
) -> Result<()> {
    std::fs::create_dir_all(inbox)
        .with_context(|| format!("failed to create {}", inbox.display()))?;
    let imported = inbox.join("imported");
    let failed = inbox.join("failed");

    println!("watching {} (ctrl-c to stop)", inbox.display());
    loop {
        let mut files: Vec<PathBuf> = std::fs::read_dir(inbox)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();

        for file in files {
            let outcome = std::fs::read_to_string(&file)
                .map_err(anyhow::Error::from)
                .and_then(|raw| cards::load_str(&raw, model.is_some()));

            let entries = match outcome {
                Ok(entries) => entries,
                Err(e) => {
                    if !is_stale(&file) {
                        continue;
                    }
                    eprintln!("{}: {e:#}; moving to failed/", file.display());
                    archive(&file, &failed)?;
                    continue;
                }
            };

            let client = AnkiClient::default();
            match ensure_non_empty(&entries)
                .and_then(|()| push(&client, &entries, deck, tags, source, model))
            {
                Ok((added, skipped)) => {
                    println!(
                        "{}: {} added, {skipped} skipped (duplicates)",
                        file.display(),
                        added
                    );
                    archive(&file, &imported)?;
                }
                Err(e) => {
                    eprintln!("{}: {e:#}; moving to failed/", file.display());
                    archive(&file, &failed)?;
                }
            }
        }

        std::thread::sleep(Duration::from_secs(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_model_is_supported_by_import_and_watch() {
        for command in ["import", "watch"] {
            let cli = Cli::try_parse_from([
                "anki-auto",
                command,
                "--deck",
                "Geography",
                "--model",
                "Basic",
            ])
            .unwrap();
            match cli.command {
                Command::Import { deck, model, .. } | Command::Watch { deck, model, .. } => {
                    assert_eq!(deck, "Geography");
                    assert_eq!(model.as_deref(), Some("Basic"));
                }
                _ => panic!("unexpected command"),
            }
        }
    }

    #[test]
    fn import_accepts_sync_flag() {
        let cli = Cli::try_parse_from(["anki-auto", "import", "cards.json", "--sync"])
            .expect("CLI should parse");
        assert!(matches!(cli.command, Command::Import { sync: true, .. }));
    }

    #[test]
    fn dry_run_and_sync_flags_can_be_parsed_together() {
        let cli = Cli::try_parse_from(["anki-auto", "import", "--dry-run", "--sync"])
            .expect("CLI should parse");
        assert!(matches!(
            cli.command,
            Command::Import {
                dry_run: true,
                sync: true,
                ..
            }
        ));
    }

    #[test]
    fn sources_accepts_deck_and_limit() {
        let cli = Cli::try_parse_from([
            "anki-auto",
            "sources",
            "--deck",
            "Immersion",
            "--limit",
            "5",
        ])
        .expect("CLI should parse");
        assert!(matches!(
            cli.command,
            Command::Sources { deck, limit } if deck == "Immersion" && limit == 5
        ));
    }
}
