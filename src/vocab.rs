use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct VocabEntry {
    pub word: String,
    pub reading: String,
    pub meaning: String,
    pub sentence: String,
    #[serde(default)]
    pub sentence_meaning: Option<String>,
}

pub fn source_tag(source: &str) -> String {
    let slug: String = source.split_whitespace().collect::<Vec<_>>().join("-");
    format!("src::{slug}")
}

pub fn load(path: Option<&Path>) -> Result<Vec<VocabEntry>> {
    let raw = match path {
        Some(p) => {
            std::fs::read_to_string(p).with_context(|| format!("failed to read {}", p.display()))?
        }
        None => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .context("failed to read stdin")?;
            buf
        }
    };
    load_str(&raw)
}

pub fn load_str(raw: &str) -> Result<Vec<VocabEntry>> {
    let value: Value = serde_json::from_str(raw.trim()).context("input is not valid JSON")?;
    let entries = match value {
        Value::Array(_) => serde_json::from_value(value).context("invalid vocab list"),
        Value::Object(_) => serde_json::from_value(value)
            .map(|entry: VocabEntry| vec![entry])
            .context("invalid vocab entry"),
        _ => bail!("expected a JSON object or array of objects"),
    }?;
    validate(&entries)?;
    Ok(entries)
}

fn validate(entries: &[VocabEntry]) -> Result<()> {
    for (index, entry) in entries.iter().enumerate() {
        for (field, value) in [
            ("word", &entry.word),
            ("reading", &entry.reading),
            ("meaning", &entry.meaning),
            ("sentence", &entry.sentence),
        ] {
            if value.trim().is_empty() {
                bail!("entry {} has an empty `{field}` field", index + 1);
            }
        }
        if entry
            .sentence_meaning
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            bail!(
                "entry {} has an empty `sentence_meaning`; omit it instead",
                index + 1
            );
        }
    }
    Ok(())
}

fn find_with_stem_fallback(word: &str, sentence: &str) -> Option<(usize, usize)> {
    if let Some(start) = sentence.find(word) {
        return Some((start, word.len()));
    }
    let total = word.chars().count();
    for keep in (2..total).rev() {
        let stem: String = word.chars().take(keep).collect();
        if let Some(start) = sentence.find(stem.as_str()) {
            return Some((start, stem.len()));
        }
    }
    None
}

pub fn highlight(word: &str, sentence: &str) -> String {
    match find_with_stem_fallback(word, sentence) {
        Some((start, len)) => format!(
            "{}<b>{}</b>{}",
            &sentence[..start],
            &sentence[start..start + len],
            &sentence[start + len..]
        ),
        None => sentence.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_array() {
        let entries = load(Some(Path::new("tests/fixtures/array.json"))).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].word, "勉強");
        assert_eq!(entries[1].sentence_meaning, None);
    }

    #[test]
    fn parses_single_object() {
        let entries = load(Some(Path::new("tests/fixtures/single.json"))).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].reading, "みず");
    }

    #[test]
    fn highlights_word_in_sentence() {
        assert_eq!(
            highlight("勉強", "毎日日本語を勉強しています。"),
            "毎日日本語を<b>勉強</b>しています。"
        );
    }

    #[test]
    fn highlight_missing_word_is_noop() {
        assert_eq!(highlight("犬", "猫がいる。"), "猫がいる。");
    }

    #[test]
    fn highlights_inflected_i_adjective() {
        assert_eq!(
            highlight("面白い", "この本はとても面白かったです。"),
            "この本はとても<b>面白</b>かったです。"
        );
    }

    #[test]
    fn highlights_inflected_verb() {
        assert_eq!(
            highlight("食べる", "昨日寿司を食べました。"),
            "昨日寿司を<b>食べ</b>ました。"
        );
    }

    #[test]
    fn source_tag_slugs_spaces() {
        assert_eq!(
            source_tag("Comprehensible Japanese 旅"),
            "src::Comprehensible-Japanese-旅"
        );
    }

    #[test]
    fn rejects_empty_required_fields() {
        let error =
            load_str(r#"{"word":"","reading":"みず","meaning":"water","sentence":"水を飲む。"}"#)
                .unwrap_err();
        assert!(error.to_string().contains("empty `word`"));
    }

    #[test]
    fn rejects_empty_optional_translation() {
        let error = load_str(
            r#"{"word":"水","reading":"みず","meaning":"water","sentence":"水を飲む。","sentence_meaning":" "}"#,
        )
        .unwrap_err();
        assert!(error.to_string().contains("omit it instead"));
    }
}
