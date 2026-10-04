//! Pilot regression tests for DF-PRODUCT-MF-003.
//!
//! Covers the SafeAI Model Finder catalogue additions — Qwen3.8-27B and the
//! five local Gemma4 variants — while protecting the mappings and HuggingFace
//! compatibility rules that already shipped in v1.1.10.
//!
//! The exact Ollama tags asserted here are the verified local tags for
//! qwen3.8:27b and gemma4:e2b / e4b / 12b / 26b / 31b. Cloud-only variants are
//! deliberately never mapped.

use std::collections::HashSet;

use llmfit_core::models::{Capability, LlmModel, ModelDatabase, parse_generation};
use llmfit_core::providers::{
    has_ollama_mapping, hf_name_to_ollama_candidates, is_likely_prequantized_repo,
    is_model_installed, ollama_pull_tag,
};

const QWEN38: &str = "Qwen/Qwen3.8-27B";

/// (HuggingFace catalogue name, verified local Ollama tag).
const GEMMA4_LOCAL: &[(&str, &str)] = &[
    ("google/gemma-4-E2B-it", "gemma4:e2b"),
    ("google/gemma-4-E4B-it", "gemma4:e4b"),
    ("google/gemma-4-12B-it", "gemma4:12b"),
    ("google/gemma-4-26B-A4B-it", "gemma4:26b"),
    ("google/gemma-4-31B-it", "gemma4:31b"),
];

fn db() -> ModelDatabase {
    ModelDatabase::new()
}

fn model<'a>(db: &'a ModelDatabase, name: &str) -> &'a LlmModel {
    db.get_all_models()
        .iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("catalogue must contain {name}"))
}

#[test]
fn pilot_qwen38_27b_is_in_the_embedded_catalogue() {
    let db = db();
    let m = model(&db, QWEN38);
    assert_eq!(m.architecture.as_deref(), Some("qwen3_8"));
    assert_eq!(m.parameter_count, "27.8B");
    assert!(m.context_length >= 262_144, "ctx = {}", m.context_length);
    assert!(
        m.capabilities.contains(&Capability::Vision),
        "Qwen3.8-27B is multimodal"
    );
    assert!(m.capabilities.contains(&Capability::ToolUse));
    assert!(!m.gguf_sources.is_empty(), "Qwen3.8-27B needs GGUF sources");
    assert!(!m.is_moe, "Qwen3.8-27B is the dense 27B variant");
}

#[test]
fn pilot_qwen38_27b_maps_to_verified_local_ollama_tag() {
    assert_eq!(ollama_pull_tag(QWEN38).as_deref(), Some("qwen3.8:27b"));
    assert!(has_ollama_mapping(QWEN38));
    assert_eq!(
        parse_generation(Some("qwen3_8"), "Qwen/Qwen3.8-27B"),
        Some(3.8)
    );
    assert_eq!(parse_generation(None, QWEN38), Some(3.8));
}

#[test]
fn pilot_gemma4_five_local_variants_are_in_the_catalogue() {
    let db = db();
    for (name, _) in GEMMA4_LOCAL {
        let m = model(&db, name);
        assert_eq!(m.architecture.as_deref(), Some("gemma4"), "{name}");
        assert!(
            m.capabilities.contains(&Capability::Vision),
            "{name} must expose multimodal vision"
        );
        assert!(
            m.capabilities.contains(&Capability::ToolUse),
            "{name} must expose tool use"
        );
        assert!(!m.gguf_sources.is_empty(), "{name} needs GGUF sources");
    }
}

#[test]
fn pilot_gemma4_variants_map_to_verified_local_ollama_tags() {
    for (name, tag) in GEMMA4_LOCAL {
        assert_eq!(ollama_pull_tag(name).as_deref(), Some(*tag), "{name}");
        assert!(has_ollama_mapping(name), "{name} must be pullable locally");
        assert!(
            !tag.ends_with("-cloud"),
            "{tag} must be a local (non-cloud) tag"
        );
    }
}

#[test]
fn pilot_gemma4_moe_fitting_uses_active_parameters() {
    let db = db();
    let m = model(&db, "google/gemma-4-26B-A4B-it");
    assert!(m.is_moe, "gemma-4-26B-A4B must be MoE");
    assert_eq!(m.active_parameters, Some(4_000_000_000));
    assert!(
        m.active_parameters.unwrap() < m.parameters_raw.unwrap(),
        "active parameters must be smaller than the true total"
    );

    let active = m.moe_active_vram_gb().expect("MoE active VRAM estimate");
    let full = m.estimate_memory_gb("Q4_K_M", m.context_length);
    assert!(
        active < full,
        "MoE active footprint {active} should be below the full {full}"
    );
    assert!(
        m.moe_offloaded_ram_gb().unwrap_or(0.0) > 0.0,
        "inactive experts must report offloadable RAM"
    );
}

#[test]
fn pilot_gemma4_dense_variants_are_not_moe() {
    let db = db();
    for (name, _) in GEMMA4_LOCAL {
        if *name == "google/gemma-4-26B-A4B-it" {
            continue;
        }
        let m = model(&db, name);
        assert!(!m.is_moe, "{name} must remain a dense model");
        assert!(
            m.moe_active_vram_gb().is_none(),
            "{name} must not use the MoE path"
        );
    }
}

#[test]
fn pilot_existing_ollama_mappings_are_preserved() {
    let preserved = [
        ("meta-llama/Llama-3.1-8B-Instruct", "llama3.1:8b"),
        ("google/gemma-3-12b-it", "gemma3:12b"),
        ("google/gemma-2-9b-it", "gemma2:9b"),
        ("Qwen/Qwen3-32B", "qwen3:32b"),
        ("Qwen/Qwen2.5-7B-Instruct", "qwen2.5:7b"),
        ("deepseek-ai/DeepSeek-R1-Distill-Qwen-7B", "deepseek-r1:7b"),
    ];
    for (name, tag) in preserved {
        assert_eq!(ollama_pull_tag(name).as_deref(), Some(tag), "{name}");
    }
}

#[test]
fn pilot_huggingface_compatibility_boundary_is_intact() {
    // Pre-quantized non-MLX formats stay excluded from fabricated MLX guesses.
    assert!(is_likely_prequantized_repo("model-awq"));
    assert!(is_likely_prequantized_repo("model-gptq-int4"));
    assert!(!is_likely_prequantized_repo("model-mlx-4bit"));

    // Unmapped models still fall back to convention candidates (no false
    // negatives) without the six new models relying on that heuristic.
    assert!(!hf_name_to_ollama_candidates("acme/Unknown-Model-1B").is_empty());

    // Installed detection still matches exact and variant tags through the
    // explicit mapping.
    let mut installed = HashSet::new();
    installed.insert("gemma4:12b-instruct-q4_k_m".to_string());
    assert!(is_model_installed("google/gemma-4-12B-it", &installed));
}
