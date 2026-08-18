use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, LazyLock};

use axum::{
    Json, Router,
    extract::{ConnectInfo, Path, Query, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE},
    },
    response::{IntoResponse, Response},
    routing::get,
};
use llmfit_core::ModelProvider;
use llmfit_core::bench::{BenchResult, bench_ollama};
use llmfit_core::fit::{
    DEFAULT_ESTIMATION_CTX, FitLevel, ModelFit, SortColumn, backend_compatible,
    rank_models_by_fit_opts_col,
};
use llmfit_core::hardware::{GpuBackend, SystemSpecs};
use llmfit_core::models::{Capability, LlmModel, QUANT_HIERARCHY, UseCase, quant_speed_multiplier};
use llmfit_core::plan::{PlanRequest, estimate_model_plan};
use llmfit_core::providers::{OllamaProvider, PullEvent};
use serde::{Deserialize, Serialize};

include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));

static ASSET_MAP: LazyLock<HashMap<&'static str, &'static EmbeddedAsset>> =
    LazyLock::new(|| EMBEDDED_WEB_ASSETS.iter().map(|a| (a.path, a)).collect());

// ── App State ──────────────────────────────────────────────────

pub struct AppState {
    #[allow(dead_code)] // Reserved for an explicit middleware layer in M2; the
    // explicit per-handler check below is the active gate.
    pub session_token: String,
    pub specs: SystemSpecs,
    pub models: Vec<LlmModel>,
    pub active_download: tokio::sync::RwLock<Option<ActiveDownload>>,
    pub download_counter: std::sync::atomic::AtomicU32,
    pub active_benchmark: tokio::sync::RwLock<Option<ActiveBenchmark>>,
    pub benchmark_counter: std::sync::atomic::AtomicU32,
}

pub struct ActiveBenchmark {
    pub id: String,
    pub model: String,
    pub status: String, // running | done | error
    pub done: usize,
    pub total: usize,
    pub result: Option<BenchResult>,
    pub error: Option<String>,
}

pub struct ActiveDownload {
    pub id: String,
    pub model_name: String,
    pub ollama_tag: String,
    pub status: String,
    pub progress_pct: f64,
    pub message: String,
}

// ── Request / Response types ───────────────────────────────────

#[derive(Debug, Default, Deserialize)]
pub struct ModelQuery {
    pub q: Option<String>, // free-text search (Browse view)
    pub use_case: Option<String>,
    pub mode: Option<String>,     // "easy" or "advanced"
    pub sort: Option<String>,     // score | tps | mem | params | ctx | newest
    pub min_fit: Option<String>,  // perfect | good | marginal
    pub context: Option<u32>,     // context-length cap for estimation
    pub memory: Option<f64>,      // VRAM override (GB)
    pub ram: Option<f64>,         // RAM override (GB)
    pub cpu_cores: Option<usize>, // CPU core override
    pub limit: Option<usize>,     // max results (Browse)
    pub installed: Option<bool>,  // filter: only installed models
    pub caps: Option<String>, // comma-separated capability labels (Vision, Tool Use, Audio, Text-to-Speech)
    pub license: Option<String>, // licence substring filter
    pub language: Option<String>, // supported-language filter
    pub size: Option<String>, // parameter-size bucket: lt1 | 1to3 | 3to7 | 7to13 | 13to30 | 30to70 | gt70
}

#[derive(Debug, Deserialize)]
pub struct PullRequest {
    pub model: String,
    pub ollama_tag: String,
}

#[derive(Debug, Deserialize)]
pub struct BenchmarkRequest {
    pub model: String,
    #[serde(default = "default_bench_runs")]
    pub runs: usize,
}

fn default_bench_runs() -> usize {
    3
}

#[derive(Debug, Deserialize)]
pub struct PlanQuery {
    pub model: String,
    pub quant: Option<String>,
    pub context: Option<u32>,
    pub target_tps: Option<f64>,
}

/// True when `tag` looks like a safe Ollama model identifier that can be
/// handed to Ollama's `DELETE /api/delete`. Only generic identifier
/// characters are accepted: no path traversal (`..`), no edge or doubled
/// slashes, at most one `:` separator, no whitespace, and a sane length
/// cap. Presence in Ollama's own `/api/tags` list is checked separately
/// before any deletion is attempted.
pub fn validate_model_tag(tag: &str) -> bool {
    if tag.is_empty() || tag.len() > 200 {
        return false;
    }
    if tag.contains("..") || tag.contains("//") {
        return false;
    }
    if tag.starts_with('/') || tag.ends_with('/') {
        return false;
    }
    if tag.matches(':').count() > 1 || tag.starts_with(':') || tag.ends_with(':') {
        return false;
    }
    tag.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | ':'))
}

#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub cpu_name: String,
    pub total_ram_gb: f64,
    pub available_ram_gb: f64,
    pub cpu_cores: usize,
    pub has_gpu: bool,
    pub gpu_name: Option<String>,
    pub gpu_vram_gb: Option<f64>,
    pub unified_memory: bool,
    pub backend: String,
    pub os: String,
}

#[derive(Debug, Serialize)]
pub struct OllamaStatus {
    pub available: bool,
    pub installed_models: Vec<String>,
    pub model_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Recommendation {
    pub name: String,
    pub ollama_tag: String,
    pub label: String,     // "Recommended", "Faster & Lighter", "Better Quality"
    pub label_key: String, // machine key: recommended | faster | better_quality | alternative
    pub fit_level: String,
    pub description: String,
    pub parameter_count: String,
    pub quant: String,
    pub estimated_tps: f64,
    pub memory_required_gb: f64,
    pub disk_size_gb: f64,
    pub use_case: String,
    pub capabilities: Vec<String>,
    pub installed: bool,
    pub slow: bool, // estimated_tps below the Easy-Mode usability floor
    pub run_mode: String,
    pub quant_options: Vec<serde_json::Value>,
    pub license: String,
    pub context_length: u32,
    pub is_moe: bool,
    pub release_date: Option<String>,
    pub languages: Vec<String>,
    pub has_vision: bool,
    pub has_tools: bool,
    pub has_audio: bool,
    pub has_tts: bool,
    pub num_experts: Option<u32>,
    pub active_experts: Option<u32>,
    pub active_parameters: Option<u64>,
}

// ── Build router ───────────────────────────────────────────────

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(web_index))
        .route("/assets/{*path}", get(web_asset))
        .route("/health", get(health))
        .route("/api/system", get(system_info))
        .route("/api/ollama/status", get(ollama_status))
        .route("/api/ollama/models", get(ollama_installed))
        .route("/api/ollama/installed", get(ollama_installed_details))
        .route("/api/recommendations", get(recommendations))
        .route("/api/models/search", get(search_models))
        .route("/api/pulls", axum::routing::post(start_pull))
        .route("/api/pulls/{id}", get(pull_status))
        .route(
            "/api/models/{name}/readiness-test",
            axum::routing::post(readiness_test),
        )
        // Same single-segment path shape as readiness-test; the destructive
        // call to Ollama (`DELETE /api/delete`) happens inside the handler.
        .route(
            "/api/models/{name}/delete",
            axum::routing::post(delete_model_http),
        )
        .route("/api/benchmarks", axum::routing::post(start_benchmark))
        .route("/api/benchmarks/{id}", get(benchmark_status))
        .route("/api/benchmarks/history", get(benchmark_history))
        .route("/api/plan", get(model_plan))
        .route("/api/plan/search", get(plan_search))
        .route("/api/models/filter-options", get(filter_options))
        .with_state(state)
}

// ── Web asset handlers ─────────────────────────────────────────

async fn web_index() -> Response {
    serve_web_path("/ui/index.html")
}

async fn web_asset(Path(path): Path<String>) -> Response {
    let asset_path = format!("/ui/{}", path.trim_start_matches('/'));
    serve_web_path(&asset_path)
}

fn serve_web_path(path: &str) -> Response {
    let Some(asset) = ASSET_MAP.get(path).copied() else {
        // Try without /ui prefix
        let alt_path = if let Some(stripped) = path.strip_prefix("/ui/") {
            format!("/ui/{}", stripped)
        } else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let Some(asset) = ASSET_MAP.get(alt_path.as_str()).copied() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let mut response = asset.bytes.to_vec().into_response();
        response
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static(asset.content_type));
        // The UI is embedded in the binary at build time, so a stale browser
        // cache can serve an old app.js against a newer index.html and break
        // every interaction. Never let browsers cache the embedded assets.
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
        return response;
    };

    let mut response = asset.bytes.to_vec().into_response();
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(asset.content_type));
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

// ── Health ─────────────────────────────────────────────────────

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

// ── System Info ────────────────────────────────────────────────

async fn system_info(State(state): State<Arc<AppState>>) -> Json<SystemInfo> {
    let specs = &state.specs;
    Json(SystemInfo {
        cpu_name: specs.cpu_name.clone(),
        total_ram_gb: (specs.total_ram_gb * 100.0).round() / 100.0,
        available_ram_gb: (specs.available_ram_gb * 100.0).round() / 100.0,
        cpu_cores: specs.total_cpu_cores,
        has_gpu: specs.has_gpu,
        gpu_name: specs.gpu_name.clone(),
        gpu_vram_gb: specs.gpu_vram_gb.map(|v| (v * 100.0).round() / 100.0),
        unified_memory: specs.unified_memory,
        backend: specs.backend.label().to_string(),
        os: std::env::consts::OS.to_string(),
    })
}

// ── Ollama Status ──────────────────────────────────────────────

async fn ollama_status() -> Json<OllamaStatus> {
    let provider = OllamaProvider::new();
    let available = provider.is_available();
    let (installed, model_count) = if available {
        let (set, count) = provider.installed_models_counted();
        let models: Vec<String> = set.iter().cloned().collect();
        (models, count)
    } else {
        (Vec::new(), 0)
    };

    Json(OllamaStatus {
        available,
        installed_models: installed,
        model_count,
        error: if !available {
            Some("Ollama is not running. Please start Ollama and refresh.".to_string())
        } else {
            None
        },
    })
}

// ── Installed Models ───────────────────────────────────────────

async fn ollama_installed() -> Json<serde_json::Value> {
    let provider = OllamaProvider::new();
    if !provider.is_available() {
        return Json(serde_json::json!({
            "models": [],
            "error": "Ollama not available"
        }));
    }

    let models: Vec<String> = provider.installed_models().iter().cloned().collect();
    Json(serde_json::json!({
        "models": models
    }))
}

/// Per-installed-model details (disk size) straight from Ollama's `/api/tags`.
/// Follows the app's existing direct-Ollama convention (see `readiness_test`);
/// cloud-hosted entries (`size == 0`) are skipped — they are not stored
/// locally. Read-only, no session token required (same as the other GET
/// endpoints).
async fn ollama_installed_details() -> Json<serde_json::Value> {
    let url = "http://localhost:11434/api/tags";
    let resp = match ureq::get(url)
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(5)))
        .build()
        .call()
    {
        Ok(r) => r,
        Err(_) => {
            return Json(serde_json::json!({ "models": [], "error": "Ollama not available" }));
        }
    };
    match resp.into_body().read_json::<serde_json::Value>() {
        Ok(v) => {
            let models: Vec<serde_json::Value> = v
                .get("models")
                .and_then(|m| m.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| {
                            let name = m.get("name")?.as_str()?.to_string();
                            let size = m.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                            if size == 0 {
                                return None;
                            }
                            Some(serde_json::json!({ "name": name, "size_bytes": size }))
                        })
                        .collect()
                })
                .unwrap_or_default();
            Json(serde_json::json!({ "models": models }))
        }
        Err(_) => Json(serde_json::json!({
            "models": [],
            "error": "Could not read the Ollama model list"
        })),
    }
}

// ── Recommendations ────────────────────────────────────────────

/// Easy Mode never hero-recommends a model that will crawl on this
/// hardware. llmfit's `fit_level` only measures memory fit, so a 27B model
/// that fits in RAM still scores "Perfect" at ~3 tok/s on CPU. This floor
/// keeps the primary recommendation genuinely usable; slower models still
/// appear as alternatives with an honest "runs slowly" label.
const EASY_MIN_TPS: f64 = 8.0;

fn specs_with_overrides(base: &SystemSpecs, q: &ModelQuery) -> SystemSpecs {
    let mut s = base.clone();
    if let Some(vram) = q.memory {
        s = s.with_gpu_memory_override(vram);
    }
    if let Some(ram) = q.ram {
        s = s.with_ram_override(ram);
    }
    if let Some(cores) = q.cpu_cores {
        s = s.with_cpu_core_override(cores);
    }
    s
}

/// True when the only GPU present is an integrated one (shared system
/// memory). Such GPUs can't meaningfully run large models, and llmfit-core
/// reports their shared pool as VRAM, which makes big models look
/// GPU-ready at unrealistic speed.
fn has_only_integrated_gpu(specs: &SystemSpecs) -> bool {
    if !specs.has_gpu {
        return false;
    }
    let name = specs.gpu_name.as_deref().unwrap_or("").to_lowercase();
    name.contains("integrated")
        || name.contains("iris")
        || name.contains("uhd")
        || name.contains("tiger")
        || name.contains("xe graphics")
        || name.contains("radeon graphics")
        || name.contains("adreno")
}

/// A spec with the GPU removed, so analysis uses the honest CPU path.
fn cpu_only_specs(base: &SystemSpecs) -> SystemSpecs {
    let mut s = base.clone();
    s.has_gpu = false;
    s.gpu_vram_gb = None;
    s.total_gpu_vram_gb = None;
    s.gpu_available_gb = None;
    s.gpu_name = None;
    s.gpu_count = 0;
    s.unified_memory = false;
    s.gpus = Vec::new();
    s.backend = if base.cpu_name.to_lowercase().contains("arm") {
        GpuBackend::CpuArm
    } else {
        GpuBackend::CpuX86
    };
    s
}

/// Resolve the specs to analyze against: apply any user overrides, then —
/// unless the user explicitly simulated a discrete GPU with a VRAM override —
/// drop an integrated GPU so estimates reflect the real CPU path.
fn analysis_specs(base: &SystemSpecs, q: &ModelQuery) -> SystemSpecs {
    let mut s = specs_with_overrides(base, q);
    if q.memory.is_none() && has_only_integrated_gpu(&s) {
        s = cpu_only_specs(&s);
    }
    s
}

fn parse_sort(raw: Option<&str>) -> SortColumn {
    match raw.unwrap_or("score").to_lowercase().as_str() {
        "tps" | "speed" => SortColumn::Tps,
        "mem" | "memory" => SortColumn::MemPct,
        "params" | "size" => SortColumn::Params,
        "ctx" | "context" => SortColumn::Ctx,
        _ => SortColumn::Score,
    }
}

/// Whether the requested sort is "newest" (release-date descending).
fn is_newest_sort(raw: Option<&str>) -> bool {
    raw.unwrap_or("").to_lowercase() == "newest"
}

/// Release date as a sortable string (ISO `YYYY-MM-DD` sorts correctly as
/// text); unknown dates sort last.
fn release_date_key(model: &LlmModel) -> (bool, String) {
    match model.release_date.as_deref() {
        Some(d) if !d.is_empty() => (false, d.to_string()),
        _ => (true, String::new()),
    }
}

/// Parameter-size bucket test for the Browse size filter.
fn size_bucket_matches(model: &LlmModel, bucket: &str) -> bool {
    let Some(raw) = model.parameters_raw else {
        return false;
    };
    let b = raw as f64 / 1e9;
    match bucket {
        "lt1" => b < 1.0,
        "1to3" => (1.0..3.0).contains(&b),
        "3to7" => (3.0..7.0).contains(&b),
        "7to13" => (7.0..13.0).contains(&b),
        "13to30" => (13.0..30.0).contains(&b),
        "30to70" => (30.0..70.0).contains(&b),
        "gt70" => b >= 70.0,
        _ => true,
    }
}

/// Apply the Browse-view filters that the easy interface hides behind
/// Advanced mode: installed, capability flags, licence, language, size.
fn apply_browse_filters(
    fits: &mut Vec<ModelFit>,
    query: &ModelQuery,
    installed_set: &HashSet<String>,
) {
    if query.installed == Some(true) {
        fits.retain(|f| model_installed(&f.model, installed_set));
    }
    if let Some(csv) = query.caps.as_deref() {
        let want: Vec<String> = csv
            .split(',')
            .map(|c| c.trim().to_lowercase())
            .filter(|c| !c.is_empty())
            .collect();
        if !want.is_empty() {
            fits.retain(|f| {
                let caps: Vec<String> = Capability::infer(&f.model)
                    .iter()
                    .map(|c| c.label().to_lowercase())
                    .collect();
                want.iter().all(|w| caps.iter().any(|c| c == w))
            });
        }
    }
    if let Some(lic) = query.license.as_deref() {
        let lic = lic.trim().to_lowercase();
        if !lic.is_empty() {
            fits.retain(|f| {
                f.model
                    .license
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&lic)
            });
        }
    }
    if let Some(lang) = query.language.as_deref() {
        let lang = lang.trim();
        if !lang.is_empty() {
            fits.retain(|f| {
                f.model
                    .languages
                    .iter()
                    .any(|l| l.eq_ignore_ascii_case(lang))
            });
        }
    }
    if let Some(size) = query.size.as_deref() {
        fits.retain(|f| size_bucket_matches(&f.model, size));
    }
}

fn min_fit_threshold(raw: Option<&str>) -> Option<FitLevel> {
    match raw.unwrap_or("").to_lowercase().as_str() {
        "perfect" => Some(FitLevel::Perfect),
        "good" => Some(FitLevel::Good),
        "marginal" => Some(FitLevel::Marginal),
        _ => None,
    }
}

/// True when `fit` is at least as good as `min` in the Perfect > Good >
/// Marginal > TooTight ordering.
fn fits_at_least(fit: FitLevel, min: FitLevel) -> bool {
    let rank = |lvl: FitLevel| match lvl {
        FitLevel::Perfect => 3,
        FitLevel::Good => 2,
        FitLevel::Marginal => 1,
        FitLevel::TooTight => 0,
    };
    rank(fit) >= rank(min)
}

/// Per-quantization estimates for a model so the UI can offer a quant
/// picker. Computed from llmfit-core's public helpers; `pool_gb` is the
/// memory pool the current run mode uses (VRAM for GPU, RAM otherwise).
fn quant_options(fit: &ModelFit, ctx: u32) -> Vec<serde_json::Value> {
    let pool_gb = fit.memory_available_gb;
    let base_mult = quant_speed_multiplier(&fit.best_quant);
    QUANT_HIERARCHY
        .iter()
        .map(|q| {
            let mem = fit.model.estimate_memory_gb(q, ctx);
            let mult = quant_speed_multiplier(q);
            let tps = if base_mult > 0.0 {
                fit.estimated_tps * mult / base_mult
            } else {
                fit.estimated_tps
            };
            serde_json::json!({
                "quant": q,
                "memory_gb": (mem * 100.0).round() / 100.0,
                "disk_gb": (fit.model.estimate_disk_gb(q) * 100.0).round() / 100.0,
                "tps": (tps * 10.0).round() / 10.0,
                "fits": mem <= pool_gb,
                "selected": q == &fit.best_quant,
            })
        })
        .collect()
}

fn model_installed(model: &LlmModel, installed: &HashSet<String>) -> bool {
    let model_lower = model.name.to_lowercase();
    let ollama_lower = llmfit_core::providers::ollama_pull_tag(&model.name)
        .unwrap_or_default()
        .to_lowercase();
    installed.contains(&model_lower)
        || (!ollama_lower.is_empty() && installed.contains(&ollama_lower))
}

async fn recommendations(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ModelQuery>,
) -> Json<serde_json::Value> {
    let use_case = parse_use_case(query.use_case.as_deref());
    let is_easy = query.mode.as_deref() != Some("advanced");
    let specs = analysis_specs(&state.specs, &query);
    let ctx = query.context.unwrap_or(DEFAULT_ESTIMATION_CTX);

    // Build fits for Ollama-compatible chat models.
    let mut fits: Vec<ModelFit> = state
        .models
        .iter()
        .filter(|m| backend_compatible(m, &specs))
        .filter(|m| is_ollama_compatible(m))
        .filter(|m| is_chat_model(m))
        .map(|m| ModelFit::analyze_with_context_limit(m, &specs, Some(ctx)))
        .collect();

    // Filter by use case — keep exact matches and, as a safety net, any
    // General-purpose candidate.
    fits.retain(|f| f.use_case == use_case || matches!(use_case, UseCase::General));
    if !is_easy && fits.is_empty() {
        fits = state
            .models
            .iter()
            .filter(|m| backend_compatible(m, &specs))
            .filter(|m| is_ollama_compatible(m))
            .filter(|m| is_chat_model(m))
            .map(|m| ModelFit::analyze_with_context_limit(m, &specs, Some(ctx)))
            .filter(|f| f.fit_level != FitLevel::TooTight)
            .collect();
    }

    // Easy Mode: only comfortable fit. Advanced: runnable only.
    if is_easy {
        fits.retain(|f| f.fit_level == FitLevel::Perfect || f.fit_level == FitLevel::Good);
    } else {
        fits.retain(|f| f.fit_level != FitLevel::TooTight);
    }

    // Optional min-fit filter (Advanced Mode).
    if let Some(min) = min_fit_threshold(query.min_fit.as_deref()) {
        fits.retain(|f| fits_at_least(f.fit_level, min));
    }

    fits = rank_models_by_fit_opts_col(fits, false, parse_sort(query.sort.as_deref()));

    let installed_set = get_installed_models();

    if fits.is_empty() {
        return Json(serde_json::json!({
            "recommendations": [],
            "message": "No suitable models found for this hardware",
            "mode": if is_easy { "easy" } else { "advanced" },
            "use_case": use_case_label(use_case),
        }));
    }

    // Hero: the best-scoring model that clears the Easy usability floor;
    // fall back to the fastest if nothing does.
    let hero_idx = if is_easy {
        match fits.iter().position(|f| f.estimated_tps >= EASY_MIN_TPS) {
            Some(i) => i,
            None => fits
                .iter()
                .enumerate()
                .max_by(|a, b| {
                    a.1.estimated_tps
                        .partial_cmp(&b.1.estimated_tps)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
                .unwrap_or(0),
        }
    } else {
        0
    };
    let mut recs: Vec<Recommendation> = vec![make_recommendation(
        &fits[hero_idx],
        "recommended",
        &installed_set,
        ctx,
    )];

    // Faster & Lighter: comfortable fit with the smallest memory footprint.
    let mut lighter: Vec<&ModelFit> = fits
        .iter()
        .filter(|f| f.fit_level == FitLevel::Perfect || f.fit_level == FitLevel::Good)
        .filter(|f| f.memory_required_gb < fits[hero_idx].memory_required_gb)
        .collect();
    lighter.sort_by(|a, b| {
        a.memory_required_gb
            .partial_cmp(&b.memory_required_gb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if let Some(fit) = lighter.first() {
        if fit.model.name != fits[hero_idx].model.name {
            recs.push(make_recommendation(fit, "faster", &installed_set, ctx));
        }
    }

    // Better Quality: highest quality score with comfortable fit.
    let mut better: Vec<&ModelFit> = fits
        .iter()
        .filter(|f| f.fit_level == FitLevel::Perfect || f.fit_level == FitLevel::Good)
        .filter(|f| f.score_components.quality > fits[hero_idx].score_components.quality)
        .collect();
    better.sort_by(|a, b| {
        b.score_components
            .quality
            .partial_cmp(&a.score_components.quality)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if let Some(fit) = better.first() {
        if !recs.iter().any(|r| r.name == fit.model.name) {
            recs.push(make_recommendation(
                fit,
                "better_quality",
                &installed_set,
                ctx,
            ));
        }
    }

    // Fill up to 3 with the next best.
    for fit in fits.iter() {
        if recs.len() >= 3 {
            break;
        }
        if !recs.iter().any(|r| r.name == fit.model.name) {
            recs.push(make_recommendation(fit, "alternative", &installed_set, ctx));
        }
    }

    Json(serde_json::json!({
        "recommendations": recs,
        "mode": if is_easy { "easy" } else { "advanced" },
        "use_case": use_case_label(use_case),
        "system": {
            "total_ram_gb": (specs.total_ram_gb * 100.0).round() / 100.0,
            "gpu_vram_gb": specs.gpu_vram_gb.map(|v| (v * 100.0).round() / 100.0),
            "backend": specs.backend.label(),
        },
    }))
}

fn make_recommendation(
    fit: &ModelFit,
    label: &str,
    installed: &HashSet<String>,
    ctx: u32,
) -> Recommendation {
    let ollama_tag = llmfit_core::providers::ollama_pull_tag(&fit.model.name).unwrap_or_default();
    let is_installed = model_installed(&fit.model, installed);

    let (label_str, description) = match label {
        "recommended" => (
            "Recommended".to_string(),
            format!(
                "Best overall balance for {}. Expected to run {} on this computer.",
                fit.use_case.label().to_lowercase(),
                match fit.fit_level {
                    FitLevel::Perfect => "perfectly",
                    FitLevel::Good => "comfortably",
                    _ => "adequately",
                }
            ),
        ),
        "faster" => (
            "Faster & Lighter".to_string(),
            format!(
                "Smaller memory footprint (~{:.1} GB) with good quality for everyday use.",
                fit.memory_required_gb
            ),
        ),
        "better_quality" => (
            "Better Quality".to_string(),
            format!(
                "Higher quality model for {}. May be slower but produces better results.",
                fit.use_case.label().to_lowercase()
            ),
        ),
        _ => (
            "Alternative".to_string(),
            "Another suitable option for this hardware.".to_string(),
        ),
    };

    Recommendation {
        name: fit.model.name.clone(),
        ollama_tag,
        label: label_str,
        label_key: label.to_string(),
        fit_level: format!("{:?}", fit.fit_level),
        description,
        parameter_count: fit.model.parameter_count.clone(),
        quant: fit.best_quant.clone(),
        estimated_tps: (fit.estimated_tps * 10.0).round() / 10.0,
        memory_required_gb: (fit.memory_required_gb * 100.0).round() / 100.0,
        disk_size_gb: (fit.model.estimate_disk_gb(&fit.best_quant) * 100.0).round() / 100.0,
        use_case: fit.model.use_case.clone(),
        capabilities: fit
            .model
            .capabilities
            .iter()
            .map(|c| c.label().to_string())
            .collect(),
        installed: is_installed,
        slow: fit.estimated_tps < EASY_MIN_TPS,
        run_mode: format!("{:?}", fit.run_mode),
        quant_options: quant_options(fit, ctx),
        license: fit.model.license.clone().unwrap_or_default(),
        context_length: fit.model.context_length,
        is_moe: fit.model.is_moe,
        release_date: fit.model.release_date.clone(),
        languages: fit.model.languages.clone(),
        has_vision: fit.model.capabilities.contains(&Capability::Vision),
        has_tools: fit.model.capabilities.contains(&Capability::ToolUse),
        has_audio: fit.model.capabilities.contains(&Capability::Audio),
        has_tts: fit.model.capabilities.contains(&Capability::Tts),
        num_experts: fit.model.num_experts,
        active_experts: fit.model.active_experts,
        active_parameters: fit.model.active_parameters,
    }
}

// ── Search / Browse ────────────────────────────────────────────

async fn search_models(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ModelQuery>,
) -> Json<serde_json::Value> {
    let specs = analysis_specs(&state.specs, &query);
    let ctx = query.context.unwrap_or(DEFAULT_ESTIMATION_CTX);
    let q = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase);

    let mut fits: Vec<ModelFit> = state
        .models
        .iter()
        .filter(|m| backend_compatible(m, &specs))
        .filter(|m| is_ollama_compatible(m))
        .filter(|m| match &q {
            Some(q) => {
                m.name.to_lowercase().contains(q)
                    || m.provider.to_lowercase().contains(q)
                    || m.use_case.to_lowercase().contains(q)
            }
            None => true,
        })
        .map(|m| ModelFit::analyze_with_context_limit(m, &specs, Some(ctx)))
        .collect();

    if let Some(min) = min_fit_threshold(query.min_fit.as_deref()) {
        fits.retain(|f| fits_at_least(f.fit_level, min));
    } else {
        // Consumer Browse view: hide models that can't run at all unless
        // the user explicitly asks to include them via min_fit.
        fits.retain(|f| f.fit_level != FitLevel::TooTight);
    }

    let installed_set = get_installed_models();
    apply_browse_filters(&mut fits, &query, &installed_set);

    if is_newest_sort(query.sort.as_deref()) {
        fits.sort_by(|a, b| {
            release_date_key(&b.model)
                .cmp(&release_date_key(&a.model))
                .then_with(|| b.model.name.cmp(&a.model.name))
        });
    } else {
        fits = rank_models_by_fit_opts_col(fits, false, parse_sort(query.sort.as_deref()));
    }

    let total = fits.len();
    if let Some(n) = query.limit {
        fits.truncate(n);
    }

    let results: Vec<serde_json::Value> = fits
        .iter()
        .map(|f| {
            let caps = Capability::infer(&f.model);
            serde_json::json!({
                "name": f.model.name,
                "provider": f.model.provider,
                "ollama_tag": llmfit_core::providers::ollama_pull_tag(&f.model.name).unwrap_or_default(),
                "parameter_count": f.model.parameter_count,
                "use_case": f.model.use_case,
                "fit_level": format!("{:?}", f.fit_level),
                "run_mode": format!("{:?}", f.run_mode),
                "estimated_tps": (f.estimated_tps * 10.0).round() / 10.0,
                "memory_required_gb": (f.memory_required_gb * 100.0).round() / 100.0,
                "disk_size_gb": (f.model.estimate_disk_gb(&f.best_quant) * 100.0).round() / 100.0,
                "quant": f.best_quant.clone(),
                "slow": f.estimated_tps < EASY_MIN_TPS,
                "installed": model_installed(&f.model, &installed_set),
                "quant_options": quant_options(f, ctx),
                "context_length": f.model.context_length,
                "capabilities": f.model.capabilities.iter().map(|c| c.label().to_string()).collect::<Vec<_>>(),
                "release_date": f.model.release_date.clone(),
                "languages": f.model.languages.clone(),
                "has_vision": caps.contains(&Capability::Vision),
                "has_tools": caps.contains(&Capability::ToolUse),
                "has_audio": caps.contains(&Capability::Audio),
                "has_tts": caps.contains(&Capability::Tts),
                "num_experts": f.model.num_experts,
                "active_experts": f.model.active_experts,
                "active_parameters": f.model.active_parameters,
                "license": f.model.license.clone().unwrap_or_default(),
                "is_moe": f.model.is_moe,
            })
        })
        .collect();

    Json(serde_json::json!({ "results": results, "total": total }))
}

fn is_ollama_compatible(model: &LlmModel) -> bool {
    // `ollama_pull_tag` returns the canonical Ollama tag for an HF model
    // name; an empty / missing result means there is no exact mapping and
    // we must not recommend silent model-file edits, so exclude it.
    match llmfit_core::providers::ollama_pull_tag(&model.name) {
        Some(t) if !t.is_empty() => true,
        _ => false,
    }
}

fn is_chat_model(model: &LlmModel) -> bool {
    // Exclude embedding-only models but keep plain chat LLMs that may have
    // no explicit `Capability` entries in the catalog. Embedding-only
    // models are explicitly tagged `UseCase::Embedding`.
    model.use_case != "Embedding" && !model.name.to_lowercase().contains("embedding")
}

fn get_installed_models() -> HashSet<String> {
    let provider = OllamaProvider::new();
    if provider.is_available() {
        provider.installed_models()
    } else {
        HashSet::new()
    }
}

fn parse_use_case(raw: Option<&str>) -> UseCase {
    match raw.unwrap_or("general").to_lowercase().as_str() {
        "coding" | "code" => UseCase::Coding,
        "reasoning" | "reason" => UseCase::Reasoning,
        "chat" | "assistant" | "everyday" => UseCase::Chat,
        "multimodal" | "vision" | "image" => UseCase::Multimodal,
        "writing" | "documents" => UseCase::General,
        _ => UseCase::General,
    }
}

fn use_case_label(uc: UseCase) -> &'static str {
    match uc {
        UseCase::Coding => "coding",
        UseCase::Reasoning => "reasoning",
        UseCase::Chat => "everyday assistant",
        UseCase::Multimodal => "image understanding",
        UseCase::Embedding => "embedding",
        UseCase::General => "writing & documents",
    }
}

// ── Pull / Download ────────────────────────────────────────────

async fn start_pull(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
    // Read the request body as raw bytes so auth runs BEFORE any body parsing.
    // Previously `Json<PullRequest>` extracted first, so a malformed body
    // returned 422 from the extractor instead of letting the session-token
    // check return 401. The body is now deserialised below, after auth.
    bytes: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Loopback-only
    if !addr.ip().is_loopback() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Downloads restricted to localhost" })),
        ));
    }

    // Per-launch session token — accepted from Authorization Bearer,
    // `x-smf-token:` header, or `?token=` in the URL query string. This MUST
    // run before any body deserialisation so an unauthenticated client gets
    // a 401 regardless of body shape (regression guarded by
    // `start_pull_no_token_with_malformed_body_is_401_not_422`).
    crate::security::require_session(&headers, raw_query.as_deref(), &state.session_token)?;

    // Now that the caller is authenticated, parse the body. A malformed
    // body remains a 422 — same status and error shape axum's `Json`
    // extractor produced — so authenticated clients still see useful
    // validation feedback.
    let body: PullRequest = serde_json::from_slice(&bytes).map_err(|e| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({
                "error": format!(
                    "Failed to deserialize the JSON body into the target type: {e}"
                )
            })),
        )
    })?;

    // Check no active download
    {
        let dl = state.active_download.read().await;
        if let Some(ref d) = *dl {
            if d.status == "pulling" {
                return Err((
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({ "error": "A download is already in progress" })),
                ));
            }
        }
    }

    let id = {
        let n = state
            .download_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("dl-{}", n)
    };

    {
        let mut dl = state.active_download.write().await;
        *dl = Some(ActiveDownload {
            id: id.clone(),
            model_name: body.model.clone(),
            ollama_tag: body.ollama_tag.clone(),
            status: "pulling".to_string(),
            progress_pct: 0.0,
            message: "starting".to_string(),
        });
    }

    let download_id = id.clone();
    let ollama_tag = body.ollama_tag.clone();
    let state_bg = Arc::clone(&state);

    // Bounded channel — Ollama emits many Progress events for big models.
    const CHANNEL_CAP: usize = 1024;
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<PullEvent>(CHANNEL_CAP);
    tokio::task::spawn_blocking(
        move || match OllamaProvider::new().start_pull(&ollama_tag) {
            Ok(handle) => loop {
                match handle.receiver.recv() {
                    Ok(event @ PullEvent::Progress { .. }) => {
                        if event_tx.blocking_send(event).is_err() {
                            return;
                        }
                    }
                    Ok(PullEvent::Done) => {
                        let _ = event_tx.blocking_send(PullEvent::Done);
                        return;
                    }
                    Ok(PullEvent::Error(e)) => {
                        let _ = event_tx.blocking_send(PullEvent::Error(e));
                        return;
                    }
                    Err(_) => {
                        let _ = event_tx
                            .blocking_send(PullEvent::Error("download channel closed".to_string()));
                        return;
                    }
                }
            },
            Err(e) => {
                let _ = event_tx.blocking_send(PullEvent::Error(e));
            }
        },
    );

    tokio::task::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            let mut dl = state_bg.active_download.write().await;
            if let Some(ref mut d) = *dl {
                if d.id != download_id {
                    break;
                }
                match event {
                    PullEvent::Progress { status, percent } => {
                        d.status = "pulling".to_string();
                        d.progress_pct = percent.unwrap_or(d.progress_pct);
                        d.message = status;
                    }
                    PullEvent::Done => {
                        d.status = "done".to_string();
                        d.progress_pct = 100.0;
                        d.message = "completed".to_string();
                        break;
                    }
                    PullEvent::Error(e) => {
                        d.status = "error".to_string();
                        d.message = e;
                        break;
                    }
                }
            }
        }
        // Defensive: if the stream ends without a terminal event (unexpected
        // shutdown), never leave the slot stuck in “pulling” — that would
        // block every future download until the app restarts.
        let mut dl = state_bg.active_download.write().await;
        if let Some(ref mut d) = *dl {
            if d.id == download_id && d.status == "pulling" {
                d.status = "error".to_string();
                d.message = "Download stopped unexpectedly".to_string();
            }
        }
    });

    Ok(Json(serde_json::json!({
        "id": id,
        "model": body.model,
        "ollama_tag": body.ollama_tag,
        "status": "pulling",
    })))
}

async fn pull_status(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if !addr.ip().is_loopback() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Download status restricted to localhost" })),
        ));
    }

    let dl = state.active_download.read().await;
    match dl.as_ref() {
        Some(d) if d.id == id => Ok(Json(serde_json::json!({
            "id": d.id,
            "model": d.model_name,
            "ollama_tag": d.ollama_tag,
            "status": d.status,
            "progress_pct": d.progress_pct,
            "message": d.message,
        }))),
        _ => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("no download with id '{}'", id) })),
        )),
    }
}

// ── Performance (local benchmarking) ───────────────────────────

/// Ollama base URL for benchmark requests. Follows the same convention as
/// the rest of the app + upstream llmfit: honour `OLLAMA_HOST` if set,
/// otherwise default to the local daemon.
fn ollama_bench_base() -> String {
    std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string())
}

/// Best-effort catalog estimate for an installed Ollama tag, so the UI can
/// honestly show “Estimated vs Measured”. Returns `None` when no catalog
/// family matches (the UI then shows no estimate rather than inventing one).
///
/// The tag's size suffix (e.g. `1.5b` in `qwen2.5:1.5b`) is used to pick the
/// catalogue entry with the closest parameter count — a family alone matches
/// 70B entries too, which would mislabel a tiny model's estimate.
fn estimate_tps_for_installed_tag(tag: &str, state: &AppState) -> Option<f64> {
    let family = tag.split(':').next().unwrap_or(tag).to_lowercase();
    if family.is_empty() {
        return None;
    }
    let size_hint = tag_size_hint(tag);
    let specs = analysis_specs(&state.specs, &ModelQuery::default());
    let mut candidates = state
        .models
        .iter()
        .filter(|m| is_ollama_compatible(m) && m.name.to_lowercase().contains(&family));
    let matched = match size_hint {
        Some(hint) => candidates.min_by(|a, b| {
            (a.params_b() - hint)
                .abs()
                .partial_cmp(&(b.params_b() - hint).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        None => candidates.next(),
    };
    matched.map(|m| {
        let fit = ModelFit::analyze_with_context_limit(m, &specs, Some(DEFAULT_ESTIMATION_CTX));
        fit.estimated_tps
    })
}

/// Parse the size suffix of an Ollama tag into a parameter-count hint in
/// billions: `qwen2.5:1.5b` → 1.5, `smollm2:135m` → 0.135, `llama3.1:8x7b` → 56.
fn tag_size_hint(tag: &str) -> Option<f64> {
    let suffix = tag.split(':').nth(1).unwrap_or(tag);
    let mut digits = String::new();
    let mut seen_dot = false;
    let mut unit: Option<char> = None;
    for ch in suffix.chars() {
        if ch.is_ascii_digit() || (ch == '.' && !seen_dot) {
            if unit.is_none() {
                digits.push(ch);
                if ch == '.' {
                    seen_dot = true;
                }
            }
        } else if unit.is_none() && (ch == 'b' || ch == 'm' || ch == 'x') {
            unit = Some(ch);
            if ch == 'x' {
                break; // MoE-style `8x7b`: use the total as a rough hint.
            }
        } else {
            break;
        }
    }
    let value: f64 = digits.parse().ok()?;
    let billions = match unit? {
        'm' => value / 1000.0,
        'b' => value,
        _ => value * 8.0, // `8x7b` → 56B total; good enough to match the entry
    };
    if billions <= 0.0 {
        return None;
    }
    Some(billions)
}

fn measurements_dir() -> std::path::PathBuf {
    if let Ok(d) = std::env::var("SAFEAI_MF_DATA_DIR") {
        return std::path::PathBuf::from(d);
    }
    if let Ok(x) = std::env::var("XDG_DATA_HOME") {
        return std::path::PathBuf::from(x).join("safeai-model-finder");
    }
    std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join(".local/share/safeai-model-finder"))
        .unwrap_or_else(|_| std::path::PathBuf::from(".smf-data"))
}

fn measurements_path() -> std::path::PathBuf {
    measurements_dir().join("measurements.json")
}

fn load_measurements() -> Vec<serde_json::Value> {
    let Ok(raw) = std::fs::read_to_string(measurements_path()) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Persist the latest measurement for a model in a tiny local JSON file
/// (app data dir — no database). Storage is the only state that survives a
/// restart; the entry is clearly derived from a real local run.
async fn save_measurement(tag: &str, result: &BenchResult, estimate_tps: Option<f64>) {
    let entry = serde_json::json!({
        "model": tag,
        "measured_at_unix": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0),
        "provider": "ollama",
        "num_runs": result.summary.num_runs,
        "avg_tps": (result.summary.avg_tps * 10.0).round() / 10.0,
        "min_tps": (result.summary.min_tps * 10.0).round() / 10.0,
        "max_tps": (result.summary.max_tps * 10.0).round() / 10.0,
        "avg_ttft_ms": result.summary.avg_ttft_ms.map(|v| (v * 10.0).round() / 10.0),
        "avg_total_ms": (result.summary.avg_total_ms * 10.0).round() / 10.0,
        "avg_output_tokens": (result.summary.avg_output_tokens * 10.0).round() / 10.0,
        "estimate_tps": estimate_tps.map(|v| (v * 10.0).round() / 10.0),
    });
    let mut items = load_measurements();
    items.retain(|i| i.get("model").and_then(|m| m.as_str()) != Some(tag));
    items.insert(0, entry);
    let _ = tokio::fs::create_dir_all(measurements_dir()).await;
    if let Ok(json) = serde_json::to_string_pretty(&items) {
        let _ = tokio::fs::write(measurements_path(), json).await;
    }
}

async fn start_benchmark(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
    Json(body): Json<BenchmarkRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if !addr.ip().is_loopback() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Benchmarks restricted to localhost" })),
        ));
    }
    crate::security::require_session(&headers, raw_query.as_deref(), &state.session_token)?;

    let tag = body.model.trim().to_string();
    if !validate_model_tag(&tag) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Invalid model tag" })),
        ));
    }
    let runs = body.runs.clamp(1, 10);

    let provider = OllamaProvider::new();
    if !provider.is_available() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "Ollama is not running" })),
        ));
    }
    let installed = provider.installed_models();
    if !installed.contains(&tag.to_lowercase()) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Model '{}' is not installed", tag) })),
        ));
    }

    {
        let bench = state.active_benchmark.read().await;
        if let Some(ref b) = *bench
            && b.status == "running"
        {
            return Err((
                StatusCode::CONFLICT,
                Json(serde_json::json!({ "error": "A benchmark is already running" })),
            ));
        }
    }

    let id = {
        let n = state
            .benchmark_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("bm-{}", n)
    };

    {
        let mut bench = state.active_benchmark.write().await;
        *bench = Some(ActiveBenchmark {
            id: id.clone(),
            model: tag.clone(),
            status: "running".to_string(),
            done: 0,
            total: runs,
            result: None,
            error: None,
        });
    }

    let bench_id = id.clone();
    let state_bg = Arc::clone(&state);
    let base_url = ollama_bench_base();
    let bench_tag = tag.clone();
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<(usize, usize)>(64);
    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<Result<BenchResult, String>>(1);

    tokio::task::spawn_blocking(move || {
        let result = bench_ollama(&base_url, &bench_tag, runs, &|done, total| {
            let _ = progress_tx.blocking_send((done, total));
        });
        let _ = done_tx.blocking_send(result);
    });

    tokio::task::spawn(async move {
        loop {
            tokio::select! {
                Some((done, total)) = progress_rx.recv() => {
                    let mut bench = state_bg.active_benchmark.write().await;
                    if let Some(ref mut b) = *bench {
                        if b.id != bench_id { return; }
                        b.done = done;
                        b.total = total;
                    }
                }
                Some(outcome) = done_rx.recv() => {
                    let mut bench = state_bg.active_benchmark.write().await;
                    if let Some(ref mut b) = *bench {
                        if b.id != bench_id { return; }
                        match outcome {
                            Ok(result) => {
                                b.status = "done".to_string();
                                b.done = result.summary.num_runs;
                                b.result = Some(result.clone());
                                let estimate = estimate_tps_for_installed_tag(&b.model, &state_bg);
                                let m = b.model.clone();
                                let _ = save_measurement(&m, &result, estimate).await;
                            }
                            Err(e) => {
                                b.status = "error".to_string();
                                b.error = Some(e);
                            }
                        }
                    }
                    return;
                }
            }
        }
    });

    Ok(Json(serde_json::json!({
        "id": id,
        "model": tag,
        "runs": runs,
        "status": "running",
    })))
}

async fn benchmark_status(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if !addr.ip().is_loopback() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Benchmark status restricted to localhost" })),
        ));
    }
    let bench = state.active_benchmark.read().await;
    let Some(b) = bench.as_ref() else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("no benchmark with id '{}'", id) })),
        ));
    };
    if b.id != id {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("no benchmark with id '{}'", id) })),
        ));
    }
    let mut out = serde_json::json!({
        "id": b.id,
        "model": b.model,
        "status": b.status,
        "done": b.done,
        "total": b.total,
    });
    if let Some(ref r) = b.result {
        out["summary"] = serde_json::to_value(&r.summary).unwrap_or(serde_json::Value::Null);
        out["runs"] = serde_json::to_value(&r.runs).unwrap_or(serde_json::Value::Null);
    }
    if let Some(ref e) = b.error {
        out["error"] = serde_json::Value::String(e.clone());
    }
    Ok(Json(out))
}

/// Previously measured results for this machine, persisted locally.
async fn benchmark_history() -> Json<serde_json::Value> {
    let mut items = load_measurements();
    items.sort_by(|a, b| {
        b.get("measured_at_unix")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
            .partial_cmp(
                &a.get("measured_at_unix")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0),
            )
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    items.truncate(20);
    Json(serde_json::json!({ "measurements": items }))
}

// ── Hardware Planner ───────────────────────────────────────────

async fn model_plan(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Query(query): Query<PlanQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if !addr.ip().is_loopback() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Planner restricted to localhost" })),
        ));
    }
    let q = query.model.trim().to_lowercase();
    let Some(model) = state
        .models
        .iter()
        .find(|m| m.name.to_lowercase() == q || m.name.to_lowercase().contains(&q))
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Unknown model '{}'", query.model) })),
        ));
    };
    let ctx = query.context.unwrap_or(DEFAULT_ESTIMATION_CTX).max(1);
    let request = PlanRequest {
        context: ctx,
        quant: query.quant.clone(),
        target_tps: query.target_tps,
        kv_quant: None,
    };
    let specs = analysis_specs(&state.specs, &ModelQuery::default());
    match estimate_model_plan(model, &request, &specs) {
        Ok(plan) => Ok(Json(serde_json::json!({
            "plan": plan,
            // Real machine hardware, so the UI can phrase “your computer has X”
            // in plain language for models that do not fit.
            "computer": {
                "ram_gb": (state.specs.total_ram_gb * 10.0).round() / 10.0,
                "vram_gb": state.specs.gpu_vram_gb.map(|v| (v * 10.0).round() / 10.0),
            },
        }))),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )),
    }
}

/// Planner model suggestions: a lightweight name/prov mass lookup so users
/// can target ANY model (including ones that do not fit this computer —
/// that is the point of planning). No fit filtering here.
async fn plan_search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ModelQuery>,
) -> Json<serde_json::Value> {
    let q = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase);
    let limit = query.limit.unwrap_or(12).min(30);
    let mut hits: Vec<&LlmModel> = state
        .models
        .iter()
        .filter(|m| match &q {
            Some(q) => m.name.to_lowercase().contains(q) || m.provider.to_lowercase().contains(q),
            None => true,
        })
        .collect();
    hits.sort_by(|a, b| a.name.cmp(&b.name));
    hits.truncate(limit);
    let results: Vec<serde_json::Value> = hits
        .iter()
        .map(|m| {
            serde_json::json!({
                "name": m.name,
                "parameter_count": m.parameter_count,
                "quant": m.quantization,
                "license": m.license.clone().unwrap_or_default(),
                "context_length": m.context_length,
                "is_moe": m.is_moe,
            })
        })
        .collect();
    Json(serde_json::json!({ "results": results }))
}

/// Distinct licence/language options for the Browse filter selects,
/// computed from the embedded catalogue (no external data).
async fn filter_options(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut licences: HashMap<String, usize> = HashMap::new();
    let mut languages: HashMap<String, usize> = HashMap::new();
    for m in &state.models {
        if let Some(l) = m.license.as_deref() {
            let l = l.trim();
            if !l.is_empty() {
                *licences.entry(l.to_string()).or_insert(0) += 1;
            }
        }
        for lang in &m.languages {
            let l = lang.trim();
            if !l.is_empty() {
                *languages.entry(l.to_string()).or_insert(0) += 1;
            }
        }
    }
    let mut licences: Vec<(String, usize)> = licences.into_iter().collect();
    licences.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut languages: Vec<(String, usize)> = languages.into_iter().collect();
    languages.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Json(serde_json::json!({
        "licences": licences.into_iter().take(12).map(|(l, _)| l).collect::<Vec<_>>(),
        "languages": languages.into_iter().take(12).map(|(l, _)| l).collect::<Vec<_>>(),
        "sizes": ["lt1", "1to3", "3to7", "7to13", "13to30", "30to70", "gt70"],
    }))
}

// ── Model Removal ──────────────────────────────────────────────

/// Endpoint: `POST /api/models/{tag}/delete`
///
/// Removes a locally installed model through Ollama's official
/// `DELETE /api/delete` API via the existing llmfit-core adapter
/// (`OllamaProvider::delete_model`). The tag must be one Ollama itself
/// reports in `/api/tags` (checked below), so a request can never delete a
/// different model than the one named. Same loopback + per-launch session
/// gates as every other mutating endpoint.
async fn delete_model_http(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
    Path(tag): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Loopback-only
    if !addr.ip().is_loopback() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Model removal restricted to localhost" })),
        ));
    }

    // Per-launch session token — same gate as pulls and readiness tests.
    crate::security::require_session(&headers, raw_query.as_deref(), &state.session_token)?;

    let tag = tag.trim().to_string();
    if !validate_model_tag(&tag) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Invalid model tag" })),
        ));
    }

    let provider = OllamaProvider::new();
    if !provider.is_available() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "Ollama is not running" })),
        ));
    }

    // Only tags Ollama itself reports as present can be removed; the
    // installed set is lower-cased by llmfit-core, so compare lower-cased
    // while deleting with the original casing.
    let installed = provider.installed_models();
    if !installed.contains(&tag.to_lowercase()) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Model '{}' is not installed", tag) })),
        ));
    }

    match provider.delete_model(&tag) {
        Ok(()) => Ok(Json(
            serde_json::json!({ "status": "deleted", "model": tag }),
        )),
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e })),
        )),
    }
}

// ── Readiness Test ─────────────────────────────────────────────

async fn readiness_test(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
    Path(model_name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if !addr.ip().is_loopback() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Readiness tests restricted to localhost" })),
        ));
    }

    crate::security::require_session(&headers, raw_query.as_deref(), &state.session_token)?;

    let provider = OllamaProvider::new();
    if !provider.is_available() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "Ollama is not running" })),
        ));
    }

    // Simple readiness check: send a short prompt
    let body = serde_json::json!({
        "model": model_name,
        "prompt": "Hello. Respond with exactly: OK",
        "stream": false,
        "options": {
            "num_predict": 10,
        },
    });

    let url = format!("http://localhost:11434/api/generate");
    match ureq::post(&url)
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .build()
        .send_json(&body)
    {
        Ok(resp) => {
            if resp.status() == 200 {
                Ok(Json(serde_json::json!({
                    "status": "ready",
                    "model": model_name,
                    "message": "Model is ready for use in SafeAI and Ollama",
                })))
            } else {
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(
                        serde_json::json!({ "error": format!("Model returned status {}", resp.status()) }),
                    ),
                ))
            }
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Readiness test failed: {}", e) })),
        )),
    }
}

// ── Tests ─────────────────────────────────────────────────────
//
// The removal endpoint is exercised against a tiny in-process mock Ollama
// HTTP server (std-only). These are secondary regression tests: the real
// milestone proof is the authorised deletion through the real UI on real
// Ollama.
//
// Handlers construct `OllamaProvider::new()`, which honours the
// `OLLAMA_HOST` environment variable, so each test points it at the mock.
// `set_var` is process-global, so every env-dependent test serialises on a
// shared mutex to avoid cross-thread races.
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use serde_json::{Value, json};
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    const TOKEN: &str = "test-session-token";

    /// Serialises tests that mutate `OLLAMA_HOST` (process-global state).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // ── Mock Ollama ────────────────────────────────────────────

    #[derive(Default)]
    struct MockOllama {
        delete_status: u16,
        /// Exact DELETE /api/delete request bodies received, in order.
        delete_bodies: Vec<String>,
        delete_calls: usize,
        /// Number of POST /api/generate requests received (warmup + runs).
        generate_calls: usize,
        /// Number of POST /api/pull requests received.
        pull_calls: usize,
    }

    /// Tiny HTTP/1.1 server answering `/api/tags` (from `tags`),
    /// `/api/delete` (with `delete_status`), and `/api/generate` (with a
    /// plausible Ollama timing payload). Records every DELETE body and
    /// counts generate calls.
    fn spawn_mock(tags: &[&str], delete_status: u16) -> (SocketAddr, Arc<Mutex<MockOllama>>) {
        let state = Arc::new(Mutex::new(MockOllama {
            delete_status,
            ..Default::default()
        }));
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
        let addr = listener.local_addr().unwrap();
        let tags: Vec<Value> = tags
            .iter()
            .map(|t| json!({ "name": t, "size": 123456789 }))
            .collect();
        let state_t = Arc::clone(&state);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let state_t = Arc::clone(&state_t);
                let tags = tags.clone();
                std::thread::spawn(move || {
                    if let Ok(req) = read_request(&mut stream) {
                        let mut st = state_t.lock().unwrap();
                        if req.path == "/api/delete" {
                            st.delete_calls += 1;
                            st.delete_bodies.push(req.body.clone());
                            let status = st.delete_status;
                            drop(st);
                            let _ = write_response(&mut stream, status, "{\"status\":\"ok\"}");
                        } else if req.path == "/api/generate" {
                            st.generate_calls += 1;
                            drop(st);
                            // Warmup + measured runs get an identical,
                            // plausible Ollama timing payload: 20 prompt
                            // tokens, 300 output tokens at ~100 tok/s.
                            let body = json!({
                                "model": "mock",
                                "response": "ok",
                                "prompt_eval_count": 20,
                                "eval_count": 300,
                                "eval_duration": 3_000_000_000u64,
                                "prompt_eval_duration": 800_000_000u64,
                                "total_duration": 4_200_000_000u64,
                            })
                            .to_string();
                            let _ = write_response(&mut stream, 200, &body);
                        } else if req.path == "/api/pull" {
                            st.pull_calls += 1;
                            drop(st);
                            // Stream NDJSON progress then success, staying
                            // “in progress” long enough for a concurrent
                            // request to be rejected with 409.
                            let mut body = String::new();
                            for i in 1..=3 {
                                body.push_str(
                                    &json!({
                                        "status": "downloading",
                                        "completed": i * 10,
                                        "total": 30,
                                    })
                                    .to_string(),
                                );
                                body.push('\n');
                            }
                            body.push_str(&json!({"status": "success"}).to_string());
                            body.push('\n');
                            let head = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            );
                            let _ = stream.write_all(head.as_bytes());
                            let _ = stream.flush();
                            std::thread::sleep(std::time::Duration::from_millis(150));
                            let _ = stream.write_all(body.as_bytes());
                            let _ = stream.flush();
                        } else {
                            drop(st);
                            let body = serde_json::to_string(&json!({"models": tags})).unwrap();
                            let _ = write_response(&mut stream, 200, &body);
                        }
                    }
                });
            }
        });
        (addr, state)
    }

    struct RawRequest {
        #[allow(dead_code)]
        method: String,
        path: String,
        body: String,
    }

    /// Byte offset just past the `\r\n\r\n` (or `\n\n`) that ends the
    /// HTTP head; returns the offset of the CRLF-CRLF separator itself.
    fn find_head_end(buf: &[u8]) -> Option<usize> {
        buf.windows(4)
            .position(|w| w == b"\r\n\r\n")
            .or_else(|| buf.windows(2).position(|w| w == b"\n\n"))
    }

    fn read_request(stream: &mut TcpStream) -> std::io::Result<RawRequest> {
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        // First read: headers + possibly the body.
        let n = stream.read(&mut chunk)?;
        buf.extend_from_slice(&chunk[..n]);
        let head_end = find_head_end(&buf).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "no header terminator")
        })?;
        let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
        let mut lines = head.lines();
        let request_line = lines.next().unwrap_or_default().to_string();
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("").to_string();
        let mut content_length = 0;
        for line in lines {
            let lower = line.to_lowercase();
            if let Some(v) = lower.strip_prefix("content-length:") {
                content_length = v.trim().parse().unwrap_or(0);
            }
        }
        let body_start = head_end + 4; // skip \r\n\r\n; tolerate unix endings
        while buf.len() < body_start + content_length {
            let n = stream.read(&mut chunk)?;
            buf.extend_from_slice(&chunk[..n]);
        }
        let body =
            String::from_utf8_lossy(&buf[body_start..body_start + content_length]).to_string();
        Ok(RawRequest { method, path, body })
    }

    fn write_response(stream: &mut TcpStream, status: u16, body: &str) -> std::io::Result<()> {
        let reason = if status == 200 { "OK" } else { "Error" };
        let response = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status,
            reason,
            body.len(),
            body
        );
        stream.write_all(response.as_bytes())
    }

    // ── Router harness ─────────────────────────────────────────

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            session_token: TOKEN.to_string(),
            specs: SystemSpecs::detect(),
            models: Vec::new(),
            active_download: tokio::sync::RwLock::new(None),
            download_counter: std::sync::atomic::AtomicU32::new(0),
            active_benchmark: tokio::sync::RwLock::new(None),
            benchmark_counter: std::sync::atomic::AtomicU32::new(0),
        })
    }

    fn pull_request(tag: &str, with_token: bool) -> Request<Body> {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/api/pulls")
            .header("content-type", "application/json");
        if with_token {
            builder = builder.header("authorization", format!("Bearer {TOKEN}"));
        }
        let mut req = builder
            .body(Body::from(
                json!({ "model": "Any", "ollama_tag": tag }).to_string(),
            ))
            .unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 45678))));
        req
    }

    fn delete_request(tag: &str, with_token: bool) -> Request<Body> {
        let mut builder = Request::builder()
            .method("POST")
            .uri(format!("/api/models/{tag}/delete"));
        if with_token {
            builder = builder.header("authorization", format!("Bearer {TOKEN}"));
        }
        let mut req = builder.body(Body::empty()).unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 45678))));
        req
    }

    async fn delete_via_api(
        router: axum::Router,
        tag: &str,
        with_token: bool,
    ) -> (StatusCode, Value) {
        let resp = router
            .clone()
            .oneshot(delete_request(tag, with_token))
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    /// Runs the `run` against the mock by pointing OLLAMA_HOST at it.
    async fn with_mock(addr: SocketAddr, tag: &str, with_token: bool) -> (StatusCode, Value) {
        // SAFETY: set_var is unsafe in edition 2024; it is sound here because
        // ENV_LOCK serialises every test that touches OLLAMA_HOST and nothing
        // else in this crate reads it.
        unsafe {
            std::env::set_var(
                "OLLAMA_HOST",
                format!("http://{}:{}", addr.ip(), addr.port()),
            );
        }
        let router = build_router(test_state());
        delete_via_api(router, tag, with_token).await
    }

    // ── Tag validation ─────────────────────────────────────────

    #[test]
    fn validate_model_tag_accepts_wellformed_tags() {
        for tag in [
            "qwen2.5:1.5b",
            "llama3.2:3b",
            "smollm2:135m",
            "qwen2.5:7b-instruct-q4_K_M",
            "hf.co/org/model:latest",
            "a.b-c_d:0.1.2",
        ] {
            assert!(validate_model_tag(tag), "expected {tag:?} to be valid");
        }
    }

    #[test]
    fn validate_model_tag_rejects_unsafe_tags() {
        for tag in [
            "",
            " ",
            "../etc/passwd",
            "a/../b",
            "a//b",
            "/leading",
            "trailing/",
            "a\\b",
            "a b",
            "a:b:c",
            ":start",
            "end:",
            "a#b",
            "a?b",
            "a%b",
            &"x".repeat(201),
        ] {
            assert!(!validate_model_tag(tag), "expected {tag:?} to be rejected");
        }
    }

    // ── Endpoint behaviour ─────────────────────────────────────

    #[tokio::test]
    async fn delete_sends_the_exact_tag_and_succeeds() {
        let _guard = ENV_LOCK.lock().unwrap();
        let (addr, mock) = spawn_mock(&["qwen2.5:1.5b"], 200);
        let (status, body) = with_mock(addr, "qwen2.5:1.5b", true).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "deleted");
        assert_eq!(body["model"], "qwen2.5:1.5b");
        let st = mock.lock().unwrap();
        assert_eq!(st.delete_calls, 1);
        assert_eq!(
            st.delete_bodies[0],
            json!({ "name": "qwen2.5:1.5b" }).to_string()
        );
    }

    #[test]
    fn delete_missing_model_is_404() {
        let _guard = ENV_LOCK.lock().unwrap();
        let (addr, mock) = spawn_mock(&[], 200);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (status, body) = rt.block_on(with_mock(addr, "qwen2.5:1.5b", true));
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].as_str().unwrap().contains("not installed"));
        assert_eq!(
            mock.lock().unwrap().delete_calls,
            0,
            "no delete must be attempted"
        );
    }

    #[test]
    fn delete_without_session_token_is_401() {
        let _guard = ENV_LOCK.lock().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let router = build_router(test_state());
        let (status, body) = rt.block_on(delete_via_api(router, "qwen2.5:1.5b", false));
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(body["error"].as_str().unwrap().contains("token"));
    }

    #[test]
    fn delete_with_wrong_token_is_401() {
        let _guard = ENV_LOCK.lock().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let router = build_router(test_state());
        let (status, _) = rt.block_on(async {
            let mut req = delete_request("qwen2.5:1.5b", false);
            req.headers_mut()
                .insert("authorization", "Bearer wrong-token".parse().unwrap());
            let resp = router.oneshot(req).await.unwrap();
            let status = resp.status();
            let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap();
            let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            (status, body)
        });
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn delete_invalid_tag_is_400() {
        let _guard = ENV_LOCK.lock().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let router = build_router(test_state());
        // Note: a `..` traversal tag would be normalised by the HTTP layer
        // before routing (never reaching the handler), so this uses a
        // URL-safe but invalid tag — double colon — that the handler must
        // reject. Traversal shapes are covered by `validate_model_tag`.
        let (status, body) = rt.block_on(delete_via_api(router, "a:b:c", true));
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .contains("Invalid model tag")
        );
    }

    #[test]
    fn delete_when_ollama_unavailable_is_503() {
        let _guard = ENV_LOCK.lock().unwrap();
        // Bind an ephemeral port, then drop it: connection refused.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (status, body) = rt.block_on(async {
            // SAFETY: serialised by ENV_LOCK, see with_mock.
            unsafe {
                std::env::set_var("OLLAMA_HOST", format!("http://127.0.0.1:{}", addr.port()));
            }
            let router = build_router(test_state());
            delete_via_api(router, "qwen2.5:1.5b", true).await
        });
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body["error"].as_str().unwrap().contains("Ollama"));
    }

    #[test]
    fn delete_when_ollama_errors_is_502() {
        let _guard = ENV_LOCK.lock().unwrap();
        let (addr, _mock) = spawn_mock(&["qwen2.5:1.5b"], 500);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (status, body) = rt.block_on(with_mock(addr, "qwen2.5:1.5b", true));
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert!(body["error"].as_str().unwrap().contains("500"));
    }

    #[tokio::test]
    async fn start_pull_rejects_concurrent_downloads_and_recovers() {
        let _guard = ENV_LOCK.lock().unwrap();
        let (addr, mock) = spawn_mock(&[], 200);
        // SAFETY: serialised by ENV_LOCK.
        unsafe {
            std::env::set_var(
                "OLLAMA_HOST",
                format!("http://{}:{}", addr.ip(), addr.port()),
            );
        }
        let router = build_router(test_state());

        // First download starts.
        let (s1, b1) = respond(router.clone(), pull_request("qwen2.5:1.5b", true)).await;
        assert_eq!(s1, StatusCode::OK, "{b1}");
        assert_eq!(b1["status"], "pulling");
        let id = b1["id"].as_str().unwrap().to_string();

        // A second download while the first is pulling is rejected.
        let (s2, b2) = respond(router.clone(), pull_request("smollm2:135m", true)).await;
        assert_eq!(s2, StatusCode::CONFLICT, "{b2}");
        assert!(
            b2["error"]
                .as_str()
                .unwrap()
                .contains("already in progress")
        );

        // The first download still reaches a terminal state.
        let mut done = false;
        for _ in 0..40 {
            let (s, b) = respond(router.clone(), get_request(&format!("/api/pulls/{id}"))).await;
            if s == StatusCode::OK && b["status"] == "done" {
                done = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert!(done, "first download never finished");

        // A new download is allowed afterwards (no permanent lock).
        let (s3, b3) = respond(router, pull_request("smollm2:135m", true)).await;
        assert_eq!(s3, StatusCode::OK, "{b3}");
        assert_eq!(b3["status"], "pulling");
        // The background task reaches Ollama asynchronously — poll for it.
        let mut saw_second_pull = false;
        for _ in 0..40 {
            if mock.lock().unwrap().pull_calls >= 2 {
                saw_second_pull = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert!(saw_second_pull, "second pull never reached Ollama");
    }

    /// Build a POST /api/pulls request carrying an arbitrary body string — used
    /// to exercise the auth-precedence regression. Always loopback-attached so
    /// the only gate in play is the session token. The legacy `pull_request`
    /// helper enforces the valid schema, so we need a sibling here.
    fn pull_request_with_body(body: &str, with_token: bool) -> Request<Body> {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/api/pulls")
            .header("content-type", "application/json");
        if with_token {
            builder = builder.header("authorization", format!("Bearer {TOKEN}"));
        }
        let mut req = builder.body(Body::from(body.to_string())).unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 45678))));
        req
    }

    /// Regression: when the JSON extractor would reject the body (missing field
    /// or malformed JSON), the unauthenticated request must return **401**, not
    /// **422**. Previously the axum `Json<PullRequest>` extractor ran before
    /// `require_session`, so any body-shape error leaked as 422 and the auth
    /// check never executed. Mirrors the curl reproduction in PROGRESS.md.
    #[tokio::test]
    async fn start_pull_no_token_with_malformed_body_is_401_not_422() {
        let router = build_router(test_state());

        // (1) Wrong-field schema — historically returned 422 from the
        //     Json extractor's "missing field `model`" before auth fired.
        let (s1, b1) = respond(
            router.clone(),
            pull_request_with_body(r#"{"name":"NotInstalled"}"#, false),
        )
        .await;
        assert_eq!(
            s1,
            StatusCode::UNAUTHORIZED,
            "wrong-field, no token should be 401, got {s1} body={b1}"
        );

        // (2) Malformed JSON — also previously hit 422 from the Json extractor
        //     before auth had a chance to run.
        let (s2, b2) = respond(
            router.clone(),
            pull_request_with_body(r#"{"model":"x""#, false),
        )
        .await;
        assert_eq!(
            s2,
            StatusCode::UNAUTHORIZED,
            "malformed JSON, no token should be 401, got {s2} body={b2}"
        );

        // (3) With the matching valid token, body parse errors are still
        //     reported (so a real client sending a wrong field gets a useful
        //     error rather than a silent pass). 422 content is preserved
        //     for the authenticated path.
        let (s3, b3) = respond(
            router,
            pull_request_with_body(r#"{"name":"NotInstalled"}"#, true),
        )
        .await;
        assert_eq!(
            s3,
            StatusCode::UNPROCESSABLE_ENTITY,
            "wrong-field, with token should remain 422, got {s3} body={b3}"
        );
        assert!(
            b3["error"].as_str().unwrap_or_default().contains("model")
                || b3["error"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("deserial"),
            "expected body-validation error message, got {b3}"
        );
    }

    /// Counterpart: a properly-shaped pull request with the correct token
    /// follows the normal path. This guards against any over-correction in
    /// the auth-precedence fix that would break legitimate pulls. Uses the
    /// conventional `#[test]` + `rt.block_on` shape so the existing
    /// `await_holding_lock` clippy warning stays the same as the rest of the
    /// suite (no new warning introduced — the lock is held across the
    /// synchronous `block_on`, not across an `.await`).
    #[test]
    fn start_pull_with_token_and_valid_body_still_returns_200() {
        let _guard = ENV_LOCK.lock().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (addr, _mock) = spawn_mock(&[], 200);
        // SAFETY: serialised by ENV_LOCK.
        unsafe {
            std::env::set_var(
                "OLLAMA_HOST",
                format!("http://{}:{}", addr.ip(), addr.port()),
            );
        }
        let router = build_router(test_state());
        let (status, body) =
            rt.block_on(async { respond(router, pull_request("qwen2.5:1.5b", true)).await });
        assert_eq!(
            status,
            StatusCode::OK,
            "valid pull should remain 200, got {status} body={body}"
        );
        assert_eq!(body["status"], "pulling");
        assert_eq!(body["ollama_tag"], "qwen2.5:1.5b");
    }

    // ── Performance (local benchmarking) ───────────────────────

    fn bench_request(with_token: bool) -> Request<Body> {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/api/benchmarks")
            .header("content-type", "application/json");
        if with_token {
            builder = builder.header("authorization", format!("Bearer {TOKEN}"));
        }
        let mut req = builder
            .body(Body::from(json!({ "model": "qwen2.5:1.5b" }).to_string()))
            .unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 45678))));
        req
    }

    fn get_request(uri: &str) -> Request<Body> {
        let mut req = Request::builder().uri(uri).body(Body::empty()).unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 45678))));
        req
    }

    async fn respond(router: axum::Router, req: Request<Body>) -> (StatusCode, Value) {
        let resp = router.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }

    fn bench_env(addr: SocketAddr) -> std::path::PathBuf {
        // SAFETY: serialised by ENV_LOCK (see with_mock).
        unsafe {
            std::env::set_var(
                "OLLAMA_HOST",
                format!("http://{}:{}", addr.ip(), addr.port()),
            );
        }
        let d = std::env::temp_dir().join(format!("smf-test-data-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        // SAFETY: serialised by ENV_LOCK.
        unsafe {
            std::env::set_var("SAFEAI_MF_DATA_DIR", &d);
        }
        d
    }

    fn plan_state() -> Arc<AppState> {
        let db = llmfit_core::models::ModelDatabase::new();
        Arc::new(AppState {
            session_token: TOKEN.to_string(),
            specs: SystemSpecs::detect(),
            models: db.get_all_models().clone(),
            active_download: tokio::sync::RwLock::new(None),
            download_counter: std::sync::atomic::AtomicU32::new(0),
            active_benchmark: tokio::sync::RwLock::new(None),
            benchmark_counter: std::sync::atomic::AtomicU32::new(0),
        })
    }

    fn urlenc(s: &str) -> String {
        s.replace(' ', "%20")
    }

    #[tokio::test]
    async fn benchmark_runs_polls_and_persists() {
        let _guard = ENV_LOCK.lock().unwrap();
        let (addr, mock) = spawn_mock(&["qwen2.5:1.5b"], 200);
        bench_env(addr);
        let state = test_state();
        let router = build_router(Arc::clone(&state));
        let (status, body) = respond(router.clone(), bench_request(true)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let id = body["id"].as_str().unwrap().to_string();

        let mut done = false;
        for _ in 0..80 {
            {
                let b = state.active_benchmark.read().await;
                if let Some(b) = b.as_ref() {
                    if b.status != "running" {
                        done = true;
                        assert_eq!(b.status, "done", "error: {:?}", b.error);
                        break;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
        assert!(done, "benchmark never finished");
        {
            let b = state.active_benchmark.read().await;
            let b = b.as_ref().unwrap();
            let s = &b.result.as_ref().unwrap().summary;
            assert_eq!(s.num_runs, 3);
            assert!((s.avg_tps - 100.0).abs() < 1.0, "avg_tps = {}", s.avg_tps);
            assert_eq!(s.avg_ttft_ms, Some(800.0));
            assert_eq!(s.avg_total_ms, 4200.0);
        }
        let (status, body) = respond(
            router.clone(),
            get_request(&format!("/api/benchmarks/{id}")),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "done");
        assert_eq!(body["summary"]["num_runs"], 3);
        // Warmup + 3 measured runs.
        assert_eq!(mock.lock().unwrap().generate_calls, 4);
        // Persisted (local JSON) and exposed via history.
        let (status2, hist) = respond(router.clone(), get_request("/api/benchmarks/history")).await;
        assert_eq!(status2, StatusCode::OK);
        let ms = hist["measurements"].as_array().unwrap();
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0]["model"], "qwen2.5:1.5b");
        assert_eq!(ms[0]["avg_tps"], 100.0);
    }

    #[tokio::test]
    async fn benchmark_without_session_token_is_401() {
        let _guard = ENV_LOCK.lock().unwrap();
        let router = build_router(test_state());
        let (status, _) = respond(router.clone(), bench_request(false)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn benchmark_missing_model_is_404() {
        let _guard = ENV_LOCK.lock().unwrap();
        let (addr, _mock) = spawn_mock(&[], 200);
        bench_env(addr);
        let router = build_router(test_state());
        let (status, body) = respond(router, bench_request(true)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["error"].as_str().unwrap().contains("not installed"));
    }

    #[tokio::test]
    async fn benchmark_ollama_down_is_503() {
        let _guard = ENV_LOCK.lock().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        bench_env(addr);
        let router = build_router(test_state());
        let (status, _) = respond(router, bench_request(true)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    // ── Hardware Planner ──────────────────────────────────────

    #[tokio::test]
    async fn plan_returns_plan_and_computer() {
        let state = plan_state();
        let router = build_router(Arc::clone(&state));
        let model_name = state
            .models
            .iter()
            .find(|m| m.name.contains("Qwen2.5-1.5B-Instruct"))
            .map(|m| m.name.clone())
            .expect("catalogue has Qwen2.5-1.5B-Instruct");
        let (status, body) = respond(
            router,
            get_request(&format!("/api/plan?model={}", urlenc(&model_name))),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let plan = &body["plan"];
        assert!(plan["minimum"]["ram_gb"].as_f64().unwrap() > 0.0);
        assert!(plan["recommended"]["ram_gb"].as_f64().unwrap() > 0.0);
        assert!(plan["run_paths"].as_array().unwrap().len() >= 3);
        assert_eq!(plan["model_name"], model_name);
        assert!(body["computer"]["ram_gb"].as_f64().unwrap() > 0.0);
    }

    #[tokio::test]
    async fn plan_unknown_model_is_404_and_bad_quant_is_400() {
        let state = plan_state();
        let router = build_router(Arc::clone(&state));
        let (s404, _) = respond(
            router.clone(),
            get_request("/api/plan?model=definitely-not-a-model-xyz"),
        )
        .await;
        assert_eq!(s404, StatusCode::NOT_FOUND);
        let model_name = state
            .models
            .iter()
            .find(|m| m.name.contains("Qwen2.5-1.5B-Instruct"))
            .map(|m| m.name.clone())
            .unwrap();
        let (s400, _) = respond(
            router,
            get_request(&format!(
                "/api/plan?model={}&quant=BOGUS",
                urlenc(&model_name)
            )),
        )
        .await;
        assert_eq!(s400, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn plan_search_suggests_catalog_models() {
        let state = plan_state();
        let router = build_router(state);
        let (status, body) = respond(router, get_request("/api/plan/search?q=qwen&limit=5")).await;
        assert_eq!(status, StatusCode::OK);
        let results = body["results"].as_array().unwrap();
        assert!(!results.is_empty(), "no qwen suggestions");
        assert!(
            results[0]["name"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("qwen")
        );
    }

    // ── Browse filters (model-level) ──────────────────────────

    fn make_model(name: &str, params_b: f64, quant: &str, extra: Value) -> LlmModel {
        let mut v = json!({
            "name": name,
            "provider": "test-provider",
            "parameter_count": format!("{}B", (params_b * 10.0) as u64 / 10),
            "parameters_raw": (params_b * 1e9) as u64,
            "min_ram_gb": 4.0,
            "recommended_ram_gb": 8.0,
            "quantization": quant,
            "context_length": 8192,
            "use_case": "Chat",
        });
        if let Some(obj) = extra.as_object() {
            for (k, val) in obj {
                v[k] = val.clone();
            }
        }
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn browse_filters_apply_correctly() {
        let specs = SystemSpecs::detect();
        let models = vec![
            make_model(
                "Test-Vision-7B-Instruct",
                7.2,
                "Q4_K_M",
                json!({
                    "capabilities": ["vision"],
                    "license": "Apache-2.0",
                    "languages": ["English"],
                    "release_date": "2024-01-01",
                }),
            ),
            make_model(
                "Test-Llama-3-3B-Instruct",
                3.2,
                "Q4_K_M",
                json!({
                    "license": "Llama 3 Community License Agreement",
                    "languages": ["English", "Spanish"],
                }),
            ),
            make_model(
                "Test-Tiny-0.5B",
                0.5,
                "Q4_K_M",
                json!({
                    "license": "Apache-2.0",
                    "languages": ["English"],
                }),
            ),
        ];
        let fits: Vec<ModelFit> = models
            .iter()
            .map(|m| ModelFit::analyze_with_context_limit(m, &specs, Some(4096)))
            .collect();

        // Installed-only.
        let mut f = fits.clone();
        let mut inst = HashSet::new();
        inst.insert("test-tiny-0.5b".to_string());
        apply_browse_filters(
            &mut f,
            &ModelQuery {
                installed: Some(true),
                ..Default::default()
            },
            &inst,
        );
        assert_eq!(f.len(), 1);
        assert!(f[0].model.name.contains("Tiny"));

        // Vision capability.
        let mut f = fits.clone();
        apply_browse_filters(
            &mut f,
            &ModelQuery {
                caps: Some("Vision".into()),
                ..Default::default()
            },
            &HashSet::new(),
        );
        assert_eq!(f.len(), 1);
        assert!(f[0].model.name.contains("Vision"));

        // Tool use (inferred for llama-3 *Instruct*).
        let mut f = fits.clone();
        apply_browse_filters(
            &mut f,
            &ModelQuery {
                caps: Some("Tool Use".into()),
                ..Default::default()
            },
            &HashSet::new(),
        );
        assert_eq!(f.len(), 1);
        assert!(f[0].model.name.contains("Llama"));

        // Licence: case-insensitive substring.
        let mut f = fits.clone();
        apply_browse_filters(
            &mut f,
            &ModelQuery {
                license: Some("apache".into()),
                ..Default::default()
            },
            &HashSet::new(),
        );
        assert_eq!(f.len(), 2);

        // Language.
        let mut f = fits.clone();
        apply_browse_filters(
            &mut f,
            &ModelQuery {
                language: Some("Spanish".into()),
                ..Default::default()
            },
            &HashSet::new(),
        );
        assert_eq!(f.len(), 1);
        assert!(f[0].model.name.contains("Llama"));

        // Size buckets.
        let mut f = fits.clone();
        apply_browse_filters(
            &mut f,
            &ModelQuery {
                size: Some("lt1".into()),
                ..Default::default()
            },
            &HashSet::new(),
        );
        assert_eq!(f.len(), 1);
        assert!(f[0].model.name.contains("Tiny"));
        let mut f = fits.clone();
        apply_browse_filters(
            &mut f,
            &ModelQuery {
                size: Some("3to7".into()),
                ..Default::default()
            },
            &HashSet::new(),
        );
        assert_eq!(f.len(), 1);
        assert!(f[0].model.name.contains("Llama"));
    }
}
