#[cfg(target_os = "macos")]
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::cards::Card;

pub const MODEL_NAME: &str = "jp-vocab";

#[derive(Debug, PartialEq, Eq)]
pub struct ImportSummary {
    pub added: usize,
    pub skipped: usize,
    pub failed_entries: Vec<usize>,
}

impl ImportSummary {
    pub fn ensure_success(&self) -> Result<()> {
        if !self.failed_entries.is_empty() {
            bail!(
                "{} added, {} duplicates skipped, {} failed: Anki rejected input entries {:?}. Check their content and note templates in Anki. Successfully added notes are already local; after fixing the input, rerun the import (existing notes will be skipped). AnkiConnect addNotes does not provide individual rejection reasons.",
                self.added,
                self.skipped,
                self.failed_entries.len(),
                self.failed_entries
            );
        }
        Ok(())
    }
}

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

    pub fn model_fields(&self, model: &str) -> Result<Vec<String>> {
        let result = self.invoke("modelFieldNames", json!({ "modelName": model }))?;
        serde_json::from_value(result).context("unexpected modelFieldNames response")
    }

    fn existing_keys(
        &self,
        deck: &str,
        model: &str,
        field: &str,
    ) -> Result<std::collections::HashSet<String>> {
        let query = format!("deck:{} note:{}", search_quote(deck), search_quote(model));
        let ids: Vec<i64> =
            serde_json::from_value(self.invoke("findNotes", json!({ "query": query }))?)
                .context("unexpected findNotes response")?;
        let mut keys = std::collections::HashSet::new();
        // Bound the size of notesInfo requests for larger decks.
        for chunk in ids.chunks(500) {
            let result = self.invoke("notesInfo", json!({ "notes": chunk }))?;
            keys.extend(note_keys(result, field)?);
        }
        Ok(keys)
    }

    pub fn add_notes(
        &self,
        deck: &str,
        model: &str,
        key_field: &str,
        tags: &[String],
        source: Option<&str>,
        entries: &[Card],
    ) -> Result<ImportSummary> {
        let mut seen = self.existing_keys(deck, model, key_field)?;
        let mut pending: Vec<(usize, &Card)> = Vec::new();
        let mut skipped = 0usize;

        for (index, e) in entries.iter().enumerate() {
            if !seen.insert(e[key_field].clone()) {
                skipped += 1;
                continue;
            }
            pending.push((index + 1, e));
        }

        if pending.is_empty() {
            return Ok(ImportSummary {
                added: 0,
                skipped,
                failed_entries: Vec::new(),
            });
        }

        let batch_source_tag = source
            .filter(|value| !value.trim().is_empty())
            .map(crate::vocab::source_tag);
        let notes: Vec<Value> = pending
            .iter()
            .map(|(_, e)| {
                let mut note_tags = tags.to_vec();
                if let Some(source_tag) = &batch_source_tag {
                    note_tags.push(source_tag.clone());
                }
                note_payload(deck, model, e, note_tags)
            })
            .collect();

        let result = self.invoke("addNotes", json!({ "notes": notes }))?;
        summarize_import(
            result,
            &pending.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
            skipped,
        )
    }
}

fn summarize_import(
    result: Value,
    input_indices: &[usize],
    skipped: usize,
) -> Result<ImportSummary> {
    let results = result.as_array().context(
        "unexpected addNotes response: expected an array; some notes may already be local",
    )?;
    if results.len() != input_indices.len()
        || results
            .iter()
            .any(|value| !value.is_null() && value.as_i64().is_none_or(|id| id <= 0))
    {
        bail!(
            "unexpected addNotes response: invalid result count or note IDs; some notes may already be local"
        );
    }
    Ok(ImportSummary {
        added: results.iter().filter(|value| !value.is_null()).count(),
        skipped,
        failed_entries: results
            .iter()
            .zip(input_indices)
            .filter_map(|(value, index)| value.is_null().then_some(*index))
            .collect(),
    })
}

fn search_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn note_keys(notes: Value, field: &str) -> Result<std::collections::HashSet<String>> {
    notes
        .as_array()
        .context("unexpected notesInfo response: expected an array")?
        .iter()
        .map(|note| {
            note.get("fields")
                .and_then(|fields| fields.get(field))
                .and_then(|field| field.get("value"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .context("unexpected notesInfo response: missing duplicate field value")
        })
        .collect()
}

fn note_payload(deck: &str, model: &str, fields: &Card, tags: Vec<String>) -> Value {
    json!({
        "deckName": deck,
        "modelName": model,
        "fields": fields,
        "tags": tags,
        "options": {
            "allowDuplicate": false,
            "duplicateScope": "deck",
            "duplicateScopeOptions": {
                "deckName": deck,
                "checkChildren": true,
                "checkAllModels": false
            }
        },
    })
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
    fn imports_custom_cards_with_scoped_exact_duplicates_and_provenance() {
        use std::io::{BufRead, BufReader, Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            let responses = [
                json!([1]),
                json!([{ "fields": { "Front": { "value": "already present" } } }]),
                json!([42, null]),
            ];
            for result in responses {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(socket.try_clone().unwrap());
                let mut size = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        size = value.trim().parse::<usize>().unwrap();
                    }
                }
                let mut body = vec![0; size];
                reader.read_exact(&mut body).unwrap();
                requests.push(serde_json::from_slice::<Value>(&body).unwrap());
                let response = json!({ "result": result, "error": null }).to_string();
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
            }
            requests
        });
        let client = AnkiClient {
            url,
            agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(5))
                .build(),
        };
        let cards = crate::cards::load_str(
            r#"[
            {"Front":"already present","Back":"skip"},
            {"Front":"new * value","Back":"keep <b>HTML</b>"},
            {"Front":"new * value","Back":"batch duplicate"},
            {"Front":"another","Back":"Anki rejects this note"}
        ]"#,
            true,
        )
        .unwrap();
        let counts = client
            .add_notes(
                "My Deck",
                "Basic",
                "Front",
                &["study".into()],
                Some("My source"),
                &cards,
            )
            .unwrap();
        assert_eq!(
            counts,
            ImportSummary {
                added: 1,
                skipped: 2,
                failed_entries: vec![4]
            }
        );
        assert!(
            counts
                .ensure_success()
                .unwrap_err()
                .to_string()
                .contains("input entries [4]")
        );
        let requests = server.join().unwrap();
        assert_eq!(
            requests[0]["params"]["query"],
            "deck:\"My Deck\" note:\"Basic\""
        );
        let notes = requests[2]["params"]["notes"].as_array().unwrap();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0]["modelName"], "Basic");
        assert_eq!(notes[0]["fields"]["Back"], "keep <b>HTML</b>");
        assert_eq!(notes[0]["tags"], json!(["study", "src::My-source"]));
        assert_eq!(notes[0]["options"]["duplicateScope"], "deck");
    }

    #[test]
    fn reports_all_rejected_notes_as_failures() {
        let summary = summarize_import(json!([null, null]), &[2, 5], 1).unwrap();
        assert_eq!(
            summary,
            ImportSummary {
                added: 0,
                skipped: 1,
                failed_entries: vec![2, 5]
            }
        );
        assert!(summary.ensure_success().is_err());
    }

    #[test]
    fn successful_import_has_no_failures() {
        let summary = summarize_import(json!([42, 43]), &[1, 3], 1).unwrap();
        assert_eq!(
            summary,
            ImportSummary {
                added: 2,
                skipped: 1,
                failed_entries: vec![]
            }
        );
        summary.ensure_success().unwrap();
    }

    #[test]
    fn rejects_malformed_import_responses() {
        for response in [
            json!([]),
            json!(["42"]),
            json!([false]),
            json!([0]),
            json!({}),
        ] {
            assert!(summarize_import(response, &[1], 0).is_err());
        }
    }

    #[test]
    fn rejects_missing_duplicate_values_in_anki_response() {
        assert!(note_keys(json!([{ "fields": {} }]), "Front").is_err());
    }

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
