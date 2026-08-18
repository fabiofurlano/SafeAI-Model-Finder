mod api;
mod security;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use llmfit_core::hardware::SystemSpecs;
use llmfit_core::models::ModelDatabase;

use api::AppState;

const DEFAULT_PORT: u16 = 8787;
const SESSION_TOKEN_VAR: &str = "SAFEAI_MF_TOKEN";

#[tokio::main]
async fn main() {
    let port = std::env::var("SAFEAI_MF_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    // Generate or load session token
    let session_token = std::env::var(SESSION_TOKEN_VAR)
        .ok()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| {
            let token = security::generate_session_token();
            println!("Session token: {}", token);
            println!(
                "Access the UI at: http://127.0.0.1:{}/?token={}",
                port, token
            );
            token
        });

    // Detect hardware
    println!("Detecting hardware...");
    let specs = SystemSpecs::detect();

    println!("CPU: {} ({} cores)", specs.cpu_name, specs.total_cpu_cores);
    println!(
        "RAM: {:.1} GB total, {:.1} GB available",
        specs.total_ram_gb, specs.available_ram_gb
    );
    if specs.has_gpu {
        println!(
            "GPU: {} ({:.1} GB VRAM, {})",
            specs.gpu_name.as_deref().unwrap_or("unknown"),
            specs.gpu_vram_gb.unwrap_or(0.0),
            specs.backend.label(),
        );
    } else {
        println!("GPU: none (CPU-only mode)");
    }

    // Load model database
    println!("Loading model catalog...");
    let db = ModelDatabase::new();
    let all_models = db.get_all_models().clone();
    println!("Loaded {} models", all_models.len());

    let state = Arc::new(AppState {
        session_token: session_token.clone(),
        specs,
        models: all_models,
        active_download: tokio::sync::RwLock::new(None),
        download_counter: std::sync::atomic::AtomicU32::new(0),
        active_benchmark: tokio::sync::RwLock::new(None),
        benchmark_counter: std::sync::atomic::AtomicU32::new(0),
    });

    let app = api::build_router(state).layer(axum::middleware::from_fn(security::validate_host));

    // Open browser
    let url = format!("http://127.0.0.1:{}/?token={}", port, session_token);
    println!(
        "\nSafeAI Model Finder starting on http://127.0.0.1:{}",
        port
    );
    println!("Press Ctrl+C to stop\n");

    match open::that(&url) {
        Ok(()) => println!("Browser opened"),
        Err(e) => println!(
            "Could not open browser automatically: {}. Open {} manually",
            e, url
        ),
    }

    // Bind to loopback only
    let addr = SocketAddr::new(IpAddr::from([127, 0, 0, 1]), port);
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind to {}: {}", addr, e);
            eprintln!(
                "Is port {} already in use? Try setting SAFEAI_MF_PORT",
                port
            );
            std::process::exit(1);
        }
    };

    println!("Listening on http://{}/", addr);

    // Only install Ctrl+C handler if we have a terminal (not backgrounded)
    let has_terminal = unsafe { libc::isatty(libc::STDIN_FILENO) } != 0;

    let server_result = if has_terminal {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            eprintln!("\nShutting down...");
        })
        .await
    } else {
        // No terminal (backgrounded) — just serve forever
        println!(
            "Running in background mode. Kill with: kill {}",
            std::process::id()
        );
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    };

    match server_result {
        Ok(()) => eprintln!("SafeAI Model Finder stopped."),
        Err(e) => {
            eprintln!("Server error: {}", e);
            std::process::exit(1);
        }
    }
}
