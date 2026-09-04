use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::vocab::VocabEntry;

pub const MODEL_NAME: &str = "jp-vocab";

#[derive(Debug, PartialEq, Eq)]
pub struct SourceSummary {
    pub source: String,
    pub note_count: usize,
}

const FIELDS: [&str; 5] = ["Word", "Reading", "Meaning", "Sentence", "SentenceMeaning"];

const CSS: &str = r#"
.card {
  font-family: -apple-system, "Hiragino Sans", "Noto Sans JP", sans-serif;
  font-size: 22px;
  text-align: center;
  color: #333333;
  background-color: #fdfdfd;
}
.word { font-size: 48px; }
.reading { font-size: 26px; color: #0a7d32; }
.meaning { font-size: 20px; margin-top: 8px; }
.sentence { margin-top: 20px; font-size: 20px; line-height: 1.6; }
.sentence-meaning { color: #777777; font-size: 17px; font-style: italic; }
hr#answer { border: none; border-top: 1px solid #cccccc; margin: 16px 0; }
"#;

const FRONT_TEMPLATE: &str = r#"<div class="word">{{Word}}</div>"#;

const BACK_TEMPLATE: &str = r#"{{FrontSide}}
<hr id=answer>
<div class="reading">{{Reading}}</div>
<div class="meaning">{{Meaning}}</div>
<div class="sentence">{{Sentence}}</div>
{{#SentenceMeaning}}<div class="sentence-meaning">{{SentenceMeaning}}</div>{{/SentenceMeaning}}"#;

pub struct AnkiClient {
    url: String,
    agent: ureq::Agent,
}

impl Default for AnkiClient {
    fn default() -> Self {
        Self {
            url: std::env::var("ANKI_CONNECT_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8765".into()),
            agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(15))
                .build(),
        }
    }
}

impl AnkiClient {
    fn post(&self, body: Value) -> Result<Value> {
        self.agent
            .post(&self.url)
            .send_json(body)
            .with_context(|| {
                format!("failed to reach AnkiConnect at {} (is Anki running with AnkiConnect installed?)", self.url)
            })?
            .into_json()
            .context("AnkiConnect returned a non-JSON response")
    }

    fn invoke(&self, action: &str, params: Value) -> Result<Value> {
        let body = json!({ "action": action, "version": 6, "params": params });
        match self.post(body.clone()) {
            Ok(resp) => unwrap_result(action, resp),
            Err(first_err) => {
                println!("AnkiConnect unreachable ({first_err:#}); launching Anki…");
                launch_and_wait(&self.url)?;
                let resp = self
                    .post(body)
                    .context("AnkiConnect did not come up after launching Anki")?;
                unwrap_result(action, resp)
            }
        }
    }

    pub fn version(&self) -> Result<String> {
        Ok(self.invoke("version", json!({}))?.to_string())
    }

    pub fn sync(&self) -> Result<()> {
        self.invoke("sync", json!({}))?;
        Ok(())
    }

    pub fn sources(&self, deck: &str) -> Result<Vec<SourceSummary>> {
        let query = format!("deck:\"{}\" tag:src::*", deck.replace('"', ""));
        let result = self.invoke("findNotes", json!({ "query": query }))?;
        let note_ids: Vec<i64> =
            serde_json::from_value(result).context("unexpected findNotes response")?;
        if note_ids.is_empty() {
            return Ok(Vec::new());
        }

        let result = self.invoke("notesInfo", json!({ "notes": note_ids }))?;
        summarize_sources(result)
    }

    pub fn ensure_deck(&self, deck: &str) -> Result<()> {
        self.invoke("createDeck", json!({ "deck": deck }))?;
        Ok(())
    }

    pub fn model_names(&self) -> Result<Vec<String>> {
        let result = self.invoke("modelNames", json!({}))?;
        serde_json::from_value(result).context("unexpected modelNames response")
    }

    pub fn ensure_model(&self) -> Result<()> {
        if self.model_names()?.iter().any(|m| m == MODEL_NAME) {
            return Ok(());
        }
        self.invoke(
            "createModel",
            json!({
                "modelName": MODEL_NAME,
                "inOrderFields": FIELDS,
                "css": CSS,
                "isCloze": false,
                "cardTemplates": [{
                    "Name": "Word -> Meaning",
                    "Front": FRONT_TEMPLATE,
                    "Back": BACK_TEMPLATE,
                }],
            }),
        )?;
        Ok(())
    }

    fn note_exists(&self, deck: &str, word: &str) -> Result<bool> {
        let query = format!(
            "deck:\"{}\" Word:\"{}\"",
            deck.replace('"', ""),
            word.replace('"', "")
        );
        let result = self.invoke("findNotes", json!({ "query": query }))?;
        let ids: Vec<i64> =
            serde_json::from_value(result).context("unexpected findNotes response")?;
        Ok(!ids.is_empty())
    }

    pub fn add_notes(
        &self,
        deck: &str,
        tags: &[String],
        source: Option<&str>,
        entries: &[VocabEntry],
    ) -> Result<(usize, usize)> {
        let mut seen = std::collections::HashSet::new();
        let mut pending: Vec<&VocabEntry> = Vec::new();
        let mut skipped = 0usize;

        for e in entries {
            if !seen.insert(e.word.as_str()) || self.note_exists(deck, &e.word)? {
                skipped += 1;
                continue;
            }
            pending.push(e);
        }

        if pending.is_empty() {
            return Ok((0, skipped));
        }

        let batch_source_tag = source
            .filter(|value| !value.trim().is_empty())
            .map(crate::vocab::source_tag);
        let notes: Vec<Value> = pending
            .iter()
            .map(|e| {
                let mut note_tags = tags.to_vec();
                if let Some(source_tag) = &batch_source_tag {
                    note_tags.push(source_tag.clone());
                }
                json!({
                    "deckName": deck,
                    "modelName": MODEL_NAME,
                    "fields": {
                        "Word": e.word,
                        "Reading": e.reading,
                        "Meaning": e.meaning,
                        "Sentence": crate::vocab::highlight(&e.word, &e.sentence),
                        "SentenceMeaning": e.sentence_meaning.clone().unwrap_or_default(),
                    },
                    "tags": note_tags,
                    "options": { "allowDuplicate": false },
                })
            })
            .collect();

        let result = self.invoke("addNotes", json!({ "notes": notes }))?;
        let results: Vec<Value> =
            serde_json::from_value(result).context("unexpected addNotes response")?;

        let added = results.iter().filter(|r| !r.is_null()).count();
        skipped += results.len() - added;
        Ok((added, skipped))
    }
}

fn unwrap_result(action: &str, resp: Value) -> Result<Value> {
    if let Some(err) = resp.get("error").filter(|e| !e.is_null()) {
        bail!("AnkiConnect error for action `{action}`: {err}");
    }
    Ok(resp["result"].clone())
}

fn summarize_sources(notes: Value) -> Result<Vec<SourceSummary>> {
    let notes = notes
        .as_array()
        .context("unexpected notesInfo response: expected an array")?;
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for note in notes {
        let tags = note
            .get("tags")
            .and_then(Value::as_array)
            .context("unexpected notesInfo response: note has no tags array")?;
        for tag in tags.iter().filter_map(Value::as_str) {
            if tag.starts_with("src::") {
                *counts.entry(tag.to_string()).or_default() += 1;
            }
        }
    }

    let mut sources: Vec<SourceSummary> = counts
        .into_iter()
        .map(|(source, note_count)| SourceSummary { source, note_count })
        .collect();
    sources.sort_by(|a, b| {
        b.note_count
            .cmp(&a.note_count)
            .then_with(|| a.source.cmp(&b.source))
    });
    Ok(sources)
}

#[cfg(target_os = "macos")]
fn open_anki() -> Result<()> {
    Command::new("open")
        .args(["-a", "Anki"])
        .status()
        .context("failed to run `open -a Anki`")?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn open_anki() -> Result<()> {
    bail!("automatic Anki launch is only implemented on macOS")
}

fn launch_and_wait(url: &str) -> Result<()> {
    open_anki()?;
    let probe = json!({ "action": "version", "version": 6 });
    for _ in 0..120 {
        std::thread::sleep(Duration::from_millis(500));
        let client = AnkiClient {
            url: url.to_string(),
            agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(5))
                .build(),
        };
        if client.post(probe.clone()).is_ok() {
            println!("Anki is up");
            return Ok(());
        }
    }
    bail!("timed out waiting for AnkiConnect after launching Anki")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_successful_response() {
        assert_eq!(
            unwrap_result("sync", json!({ "result": null, "error": null })).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn reports_anki_connect_errors_with_action() {
        let error = unwrap_result(
            "sync",
            json!({ "result": null, "error": "sync requires authentication" }),
        )
        .unwrap_err();
        assert!(error.to_string().contains("action `sync`"));
        assert!(error.to_string().contains("requires authentication"));
    }

    #[test]
    fn summarizes_source_tags_by_frequency() {
        let summaries = summarize_sources(json!([
            { "tags": ["youtube", "src::Comprehensible-Japanese-https://youtu.be/one"] },
            { "tags": ["src::Comprehensible-Japanese-https://youtu.be/one"] },
            { "tags": ["src::Miku-Real-Japanese-https://youtu.be/two"] }
        ]))
        .unwrap();

        assert_eq!(
            summaries,
            vec![
                SourceSummary {
                    source: "src::Comprehensible-Japanese-https://youtu.be/one".into(),
                    note_count: 2,
                },
                SourceSummary {
                    source: "src::Miku-Real-Japanese-https://youtu.be/two".into(),
                    note_count: 1,
                },
            ]
        );
    }

    #[test]
    fn rejects_malformed_notes_info_response() {
        let error = summarize_sources(json!([{ "tags": null }])).unwrap_err();
        assert!(error.to_string().contains("tags array"));
    }
}
