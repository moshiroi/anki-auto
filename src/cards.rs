use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::Value;

pub type Card = BTreeMap<String, String>;

pub fn load(path: Option<&Path>, custom: bool) -> Result<Vec<Card>> {
    let raw = match path {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?,
        None => {
            let mut raw = String::new();
            std::io::stdin()
                .read_to_string(&mut raw)
                .context("failed to read stdin")?;
            raw
        }
    };
    load_str(&raw, custom)
}

pub fn load_str(raw: &str, custom: bool) -> Result<Vec<Card>> {
    if !custom {
        return Ok(crate::vocab::load_str(raw)?
            .into_iter()
            .map(|entry| {
                Card::from([
                    ("Word".into(), entry.word.clone()),
                    ("Reading".into(), entry.reading),
                    ("Meaning".into(), entry.meaning),
                    (
                        "Sentence".into(),
                        crate::vocab::highlight(&entry.word, &entry.sentence),
                    ),
                    (
                        "SentenceMeaning".into(),
                        entry.sentence_meaning.unwrap_or_default(),
                    ),
                ])
            })
            .collect());
    }
    let value: Value = serde_json::from_str(raw.trim()).context("input is not valid JSON")?;
    let values = match value {
        Value::Array(values) => values,
        Value::Object(_) => vec![value],
        _ => bail!("expected a JSON object or array of objects"),
    };
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let card: Card = serde_json::from_value(value).with_context(|| {
                format!("entry {} must be an object of string fields", index + 1)
            })?;
            if card.is_empty() || card.keys().any(|field| field.trim().is_empty()) {
                bail!("entry {} must have named fields", index + 1);
            }
            if card.values().all(|value| value.trim().is_empty()) {
                bail!("entry {} has no non-empty fields", index + 1);
            }
            Ok(card)
        })
        .collect()
}

pub fn validate_model(entries: &[Card], fields: &[String]) -> Result<()> {
    let first = fields.first().context("Anki note type has no fields")?;
    for (index, entry) in entries.iter().enumerate() {
        for field in entry.keys() {
            if !fields.contains(field) {
                bail!(
                    "entry {} has unknown field `{field}`; note type fields: {}",
                    index + 1,
                    fields.join(", ")
                );
            }
        }
        if entry.get(first).is_none_or(|value| value.trim().is_empty()) {
            bail!(
                "entry {} requires a non-empty first field `{first}` for duplicate checking",
                index + 1
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_arbitrary_fields_and_optional_empty_values() {
        let cards = load_str(
            r#"[{"Front":"hello","Back":""},{"Front":"bye","Back":"goodbye"}]"#,
            true,
        )
        .unwrap();
        validate_model(&cards, &["Front".into(), "Back".into()]).unwrap();
        assert_eq!(cards.len(), 2);
    }

    #[test]
    fn rejects_invalid_custom_input() {
        for raw in ["null", "[1]", r#"{"Front":5}"#, "{}", r#"{"Front":" "}"#] {
            assert!(load_str(raw, true).is_err(), "{raw}");
        }
    }

    #[test]
    fn validates_field_names_and_duplicate_key() {
        for raw in [
            r#"{"Front":"x","Typo":"y"}"#,
            r#"{"Back":"y"}"#,
            r#"{"Front":"","Back":"y"}"#,
        ] {
            let cards = load_str(raw, true).unwrap();
            assert!(validate_model(&cards, &["Front".into(), "Back".into()]).is_err());
        }
    }

    #[test]
    fn preserves_japanese_mapping() {
        let cards = load_str(include_str!("../tests/fixtures/single.json"), false).unwrap();
        assert_eq!(cards[0]["Word"], "水");
        assert!(cards[0]["Sentence"].contains("<b>水</b>"));
    }
}
