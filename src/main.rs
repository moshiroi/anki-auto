mod anki;
mod vocab;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::anki::AnkiClient;
use crate::vocab as v;

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
        #[arg(long, default_value = "Japanese::Vocab")]
        deck: String,
        /// Comma-separated tags for the notes
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Parse and validate only, without touching Anki
        #[arg(long)]
        dry_run: bool,
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
            dry_run,
        } => import(path.as_deref(), &deck, &tags, dry_run)?,
    }
    Ok(())
}

fn import(
    path: Option<&std::path::Path>,
    deck: &str,
    tags: &[String],
    dry_run: bool,
) -> Result<()> {
    let entries = v::load(path)?;
    if entries.is_empty() {
        anyhow::bail!("no vocab entries found in input");
    }

    if dry_run {
        println!(
            "parsed {} entr{}:",
            entries.len(),
            if entries.len() == 1 { "y" } else { "ies" }
        );
        for e in &entries {
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
        return Ok(());
    }

    let client = AnkiClient::default();
    client.ensure_deck(deck)?;
    client.ensure_model()?;
    let (added, skipped) = client.add_notes(deck, tags, &entries)?;

    println!("{}/{} added to `{deck}`", added, entries.len());
    if skipped > 0 {
        println!("{skipped} skipped (duplicates)");
    }
    Ok(())
}
