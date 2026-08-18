//! Internationalisation contract tests.
//!
//! The UI is localised through embedded locale files
//! (`src/ui/locales/{en,it}.json`). These tests guarantee:
//!
//! 1. both locale files parse as valid JSON;
//! 2. Italian and English expose exactly the same key tree (so Italian can
//!    never silently miss a UI string — the frontend falls back to English,
//!    which would hide the gap);
//! 3. structural invariants: same guide topics, same glossary entry count,
//!    and no empty string values;
//! 4. key Italian translations for the primary navigation surface.

use std::collections::BTreeSet;
use std::fs;

use serde_json::Value;

const LOCALE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/ui/locales");

fn load(lang: &str) -> Value {
    let path = format!("{LOCALE_DIR}/{lang}.json");
    let raw = fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{path} is not valid JSON: {e}"))
}

/// Collect the dotted path of every leaf (objects recurse; arrays and
/// scalars are leaves, so array *contents* do not create keys — both
/// languages must agree on structure, not on the number of paragraphs).
fn collect_keys(v: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(map) => {
            if map.is_empty() {
                out.insert(prefix.to_string());
            }
            for (k, val) in map {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                collect_keys(val, &p, out);
            }
        }
        _ => {
            out.insert(prefix.to_string());
        }
    }
}

/// Recursively verify no leaf string is empty and no key is missing a
/// value of the same JSON type as the English reference.
fn check_values(lang: &str, en: &Value, it: &Value, prefix: &str, errors: &mut Vec<String>) {
    match (en, it) {
        (Value::Object(en_map), Value::Object(it_map)) => {
            for (k, en_val) in en_map {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                match it_map.get(k) {
                    Some(it_val) => check_values(lang, en_val, it_val, &p, errors),
                    None => errors.push(format!("{lang}: missing key {p}")),
                }
            }
        }
        (Value::String(s), Value::String(s2)) => {
            if s.trim().is_empty() {
                errors.push(format!("{lang}: empty string at {prefix}"));
            }
            if s2.trim().is_empty() {
                errors.push(format!("{lang}: empty string at {prefix}"));
            }
        }
        (Value::Array(a), Value::Array(a2)) => {
            if a.len() != a2.len() {
                errors.push(format!(
                    "{lang}: array length differs at {prefix} (en={}, {lang}={})",
                    a.len(),
                    a2.len()
                ));
            }
        }
        _ => {
            if std::mem::discriminant(en) != std::mem::discriminant(it) {
                errors.push(format!("{lang}: type differs at {prefix}"));
            }
        }
    }
}

#[test]
fn locale_files_are_valid_json_and_key_parity() {
    let en = load("en");
    let it = load("it");

    assert_eq!(en["meta"]["lang"], "en", "en.json meta.lang");
    assert_eq!(it["meta"]["lang"], "it", "it.json meta.lang");

    let mut en_keys = BTreeSet::new();
    let mut it_keys = BTreeSet::new();
    collect_keys(&en, "", &mut en_keys);
    collect_keys(&it, "", &mut it_keys);

    assert_eq!(
        en_keys,
        it_keys,
        "key trees must match; missing in it: {:?}",
        en_keys.difference(&it_keys).collect::<Vec<_>>()
    );

    let mut errors = Vec::new();
    check_values("it", &en, &it, "", &mut errors);
    assert!(errors.is_empty(), "value problems: {errors:#?}");
}

#[test]
fn guide_structure_matches_between_languages() {
    let en = load("en");
    let it = load("it");

    let en_topics = en["guide"]["topics"].as_object().expect("guide.topics");
    let it_topics = it["guide"]["topics"].as_object().expect("guide.topics");
    assert_eq!(
        en_topics.keys().collect::<BTreeSet<_>>(),
        it_topics.keys().collect::<BTreeSet<_>>(),
        "guide topic ids must match"
    );

    let en_glossary = en["guide"]["glossary"]["entries"].as_array().unwrap();
    let it_glossary = it["guide"]["glossary"]["entries"].as_array().unwrap();
    assert_eq!(en_glossary.len(), it_glossary.len(), "glossary entry count");
    assert!(en_glossary.len() >= 10, "glossary should be substantive");
    assert!(en_topics.len() >= 10, "guide should have 10 topics");
}

#[test]
fn key_italian_translations_exist() {
    let it = load("it");
    assert_eq!(it["nav"]["overview"], "Panoramica");
    assert_eq!(it["nav"]["find"], "Trova un modello");
    assert_eq!(it["nav"]["browse"], "Sfoglia i modelli");
    assert_eq!(it["nav"]["installed"], "Installati");
    assert_eq!(it["nav"]["guide"], "Guida");
    assert_eq!(it["overview"]["title1"], "Trova il modello giusto");
    assert_eq!(it["rec"]["recommended"], "Consigliato");
    assert_eq!(it["guide"]["title"], "Guida");
}
