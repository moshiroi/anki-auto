use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::vocab::VocabEntry;

pub const MODEL_NAME: &str = "jp-vocab";

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
}

impl Default for AnkiClient {
    fn default() -> Self {
        Self {
            url: std::env::var("ANKI_CONNECT_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8765".into()),
        }
    }
}

impl AnkiClient {
    fn invoke(&self, action: &str, params: Value) -> Result<Value> {
        let body = json!({ "action": action, "version": 6, "params": params });
        let resp: Value = ureq::post(&self.url)
            .send_json(body)
            .with_context(|| format!("failed to reach AnkiConnect at {} (is Anki running with AnkiConnect installed?)", self.url))?
            .into_json()?;
        if let Some(err) = resp.get("error").filter(|e| !e.is_null()) {
            bail!("AnkiConnect error for action `{action}`: {err}");
        }
        Ok(resp["result"].clone())
    }

    pub fn version(&self) -> Result<String> {
        Ok(self.invoke("version", json!({}))?.to_string())
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

    pub fn add_notes(
        &self,
        deck: &str,
        tags: &[String],
        entries: &[VocabEntry],
    ) -> Result<(usize, usize)> {
        let notes: Vec<Value> = entries
            .iter()
            .map(|e| {
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
                    "tags": tags,
                    "options": { "allowDuplicate": false },
                })
            })
            .collect();

        let result = self.invoke("addNotes", json!({ "notes": notes }))?;
        let results: Vec<Value> =
            serde_json::from_value(result).context("unexpected addNotes response")?;

        let added = results.iter().filter(|r| !r.is_null()).count();
        let skipped = results.len() - added;
        Ok((added, skipped))
    }
}
