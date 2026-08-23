mod anki;
mod vocab;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use crate::anki::AnkiClient;
use crate::vocab::VocabEntry;

#[derive(Parser)]
#[command(name = "anki-auto", about = "Automate Japanese Anki card creation")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check that Anki is running and AnkiConnect is reachable
    Ping,
    /// Import vocab entries from a JSON file (or stdin if no file given)
    Import {
        /// Path to JSON file; omit to read from stdin
        path: Option<PathBuf>,
        /// Target deck (created if missing)
        #[arg(long, default_value = "Japanese")]
        deck: String,
        /// Comma-separated tags for the notes
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Provenance for this import batch, such as a video title or URL
        #[arg(long)]
        source: Option<String>,
        /// Parse and validate only, without touching Anki
        #[arg(long)]
        dry_run: bool,
    },
    /// Watch a directory and import any JSON file that appears
    Watch {
        /// Directory to watch (created if missing)
        #[arg(long, default_value = "inbox")]
        inbox: PathBuf,
        /// Target deck (created if missing)
        #[arg(long, default_value = "Japanese")]
        deck: String,
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
        Command::Import {
            path,
            deck,
            tags,
            source,
            dry_run,
        } => {
            let entries = vocab::load(path.as_deref())?;
            ensure_non_empty(&entries)?;
            if dry_run {
                print_entries(&entries);
            } else {
                report(push(&entries, &deck, &tags, source.as_deref())?);
            }
        }
        Command::Watch {
            inbox,
            deck,
            tags,
            source,
        } => watch(&inbox, &deck, &tags, source.as_deref())?,
    }
    Ok(())
}

fn ensure_non_empty(entries: &[VocabEntry]) -> Result<()> {
    if entries.is_empty() {
        bail!("no vocab entries found in input");
    }
    Ok(())
}

fn print_entries(entries: &[VocabEntry]) {
    println!(
        "parsed {} entr{}:",
        entries.len(),
        if entries.len() == 1 { "y" } else { "ies" }
    );
    for e in entries {
        println!(
            "  {} [{}] — {} ({})",
            e.word,
            e.reading,
            e.meaning,
            e.sentence_meaning
                .as_deref()
                .unwrap_or("no sentence translation")
        );
    }
}

fn push(
    entries: &[VocabEntry],
    deck: &str,
    tags: &[String],
    source: Option<&str>,
) -> Result<(usize, usize)> {
    let client = AnkiClient::default();
    client.ensure_deck(deck)?;
    client.ensure_model()?;
    client.add_notes(deck, tags, source, entries)
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

fn watch(inbox: &Path, deck: &str, tags: &[String], source: Option<&str>) -> Result<()> {
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
                .and_then(|raw| vocab::load_str(&raw));

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

            match ensure_non_empty(&entries).and_then(|()| push(&entries, deck, tags, source)) {
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
