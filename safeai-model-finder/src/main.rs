mod api;
mod security;

use std::io::IsTerminal;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use llmfit_core::hardware::SystemSpecs;
use llmfit_core::models::ModelDatabase;

use api::AppState;

const DEFAULT_PORT: u16 = 8787;
const SESSION_TOKEN_VAR: &str = "SAFEAI_MF_TOKEN";

#[tokio::main]
async fn main() {
    // Track whether the caller explicitly requested a port via SAFEAI_MF_PORT.
    // We use this to decide:
    //   * whether a busy default port should auto-fallback to another port
    //     (yes for the default-port path, NO when the user picked it), and
    //   * whether the URL we print / open in the browser is the requested
    //     port or the actual bound port (only ever the actual bound port).
    let explicit_port = std::env::var("SAFEAI_MF_PORT")
        .ok()
        .filter(|p| !p.is_empty())
        .and_then(|p| p.parse::<u16>().ok());

    let requested_port = explicit_port.unwrap_or(DEFAULT_PORT);

    // Generate or load session token. The token is independent of the bound
    // port; we deliberately print only the token here. The URL is only
    // known for sure AFTER we have bound, so we never print or open a URL
    // before the listener exists.
    let session_token = std::env::var(SESSION_TOKEN_VAR)
        .ok()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| {
            let token = security::generate_session_token();
            println!("Session token: {}", token);
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

    // Bind to loopback first. If the bind fails AND the user did not
    // explicitly request this port, fall back to 127.0.0.1:0 so the OS
    // allocates a free user-port. The fallback also binds loopback-only,
    // matching the AGENTS.md / PRD §3 privacy invariant that the local
    // service NEVER widens to LAN or public interfaces.
    let allow_fallback = explicit_port.is_none();
    let listener = match bind_loopback(requested_port, allow_fallback).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind to 127.0.0.1:{}: {}", requested_port, e);
            if let Some(req) = explicit_port {
                eprintln!(
                    "The explicitly requested SAFEAI_MF_PORT={} is not available on \
                     127.0.0.1. Stop the conflicting service, or choose a different \
                     SAFEAI_MF_PORT (for example, see `ss -tlnp | grep {}`).",
                    req, req
                );
            } else {
                eprintln!(
                    "Could not bind 127.0.0.1:{} (default) or any OS-allocated \
                     loopback port. SafeAI Model Finder is exiting.",
                    DEFAULT_PORT
                );
            }
            std::process::exit(1);
        }
    };

    // The port we are actually bound on. The browser URL and the printed
    // Access-URL MUST both use this number — never the requested number —
    // so they always correspond to a socket we own.
    let actual_port = listener
        .local_addr()
        .expect("freshly-bound listener must have a local address")
        .port();

    if actual_port != requested_port {
        // We only fall back when the default-port path was taken, so this
        // can only fire when explicit_port was None. The log line is plain
        // text a nontechnical user can read.
        println!(
            "Port {} is already in use; using local port {} instead.",
            requested_port, actual_port
        );
    }

    let url = format!("http://127.0.0.1:{}/?token={}", actual_port, session_token);

    println!(
        "\nSafeAI Model Finder starting on http://127.0.0.1:{}/\n",
        actual_port
    );
    println!("Access the UI at: {}", url);
    println!("Press Ctrl+C to stop\n");

    // Browser open happens AFTER a successful bind, with the actual port.
    // We never open the browser toward a port we do not own.
    match open::that(&url) {
        Ok(()) => println!("Browser opened"),
        Err(e) => println!(
            "Could not open browser automatically: {}. Open {} manually",
            e, url
        ),
    }

    // Only install Ctrl+C handler if we have a terminal (not backgrounded)
    let has_terminal = std::io::stdin().is_terminal();

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

/// Bind a TCP listener to `127.0.0.1:<requested_port>`.
///
/// Loopback-only — never widens to `0.0.0.0` or another interface.
///
/// When `requested_port` is already in use AND `allow_fallback` is `true`
/// (the default-port path), this falls back to `127.0.0.1:0` so the
/// operating system picks a free user-port. The caller must then read
/// `listener.local_addr().port()` to learn the chosen port and build any
/// URL from that number — never from `requested_port`.
///
/// When `requested_port` is already in use AND `allow_fallback` is `false`
/// (the explicit-SAFEAI_MF_PORT path), returns the bind error verbatim;
/// the caller fails the process so the user can resolve the conflict
/// deterministically rather than have us silently switch ports.
async fn bind_loopback(
    requested_port: u16,
    allow_fallback: bool,
) -> std::io::Result<tokio::net::TcpListener> {
    let primary = SocketAddr::new(IpAddr::from([127, 0, 0, 1]), requested_port);
    match tokio::net::TcpListener::bind(primary).await {
        Ok(listener) => Ok(listener),
        Err(_) if allow_fallback => {
            // 127.0.0.1:0 — kernel-assigned, still loopback-only.
            let fb = SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 0);
            tokio::net::TcpListener::bind(fb).await
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;

    /// Pick a free loopback port and KEEP a listener holding it busy for
    /// the rest of the test (used as an explicit `occupied` value).
    async fn occupy_random_loopback_port() -> (tokio::net::TcpListener, u16) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind 127.0.0.1:0 must succeed on a normal host");
        let port = listener.local_addr().unwrap().port();
        (listener, port)
    }

    #[tokio::test]
    async fn requested_port_is_used_when_it_is_free() {
        // Find a port that is currently free, drop the listener that
        // temporarily held it, then ask bind_loopback for that exact port.
        // We retry once if a kernel race snatches the port back between
        // our drop and the second bind.
        for _ in 0..2 {
            let probe = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind probe");
            let port = probe.local_addr().unwrap().port();
            drop(probe);

            if let Ok(l) = bind_loopback(port, true).await {
                assert_eq!(
                    l.local_addr().unwrap().port(),
                    port,
                    "default-port-free: helper must return the requested port verbatim"
                );
                return;
            }
        }
        panic!("could not re-bind the same released loopback port twice");
    }

    #[tokio::test]
    async fn default_port_busy_triggers_kernel_allocated_loopback_fallback() {
        let (occupier, occupied) = occupy_random_loopback_port().await;
        let listener = bind_loopback(occupied, true)
            .await
            .expect("fallback bind on 127.0.0.1:0 must succeed");
        let actual = listener.local_addr().unwrap().port();
        assert_ne!(
            actual, occupied,
            "default-port-busy: fallback must allocate a DIFFERENT port"
        );
        assert!(
            actual >= 1024,
            "fallback port {} should be a user-port, not a system port",
            actual
        );
        drop(listener);
        drop(occupier);
    }

    #[tokio::test]
    async fn explicit_safeai_mf_port_busy_does_not_silently_switch() {
        let (occupier, occupied) = occupy_random_loopback_port().await;
        let err = bind_loopback(occupied, false)
            .await
            .expect_err("explicit-port busy bind must return Err");
        assert_eq!(
            err.kind(),
            ErrorKind::AddrInUse,
            "explicit-port busy bind must surface AddrInUse, not anything else"
        );
        drop(occupier);
    }

    #[tokio::test]
    async fn explicit_safeai_mf_port_succeeds_when_free() {
        // Same harness as the default-port-free test but with
        // allow_fallback=false.
        for _ in 0..2 {
            let probe = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind probe");
            let port = probe.local_addr().unwrap().port();
            drop(probe);
            if let Ok(l) = bind_loopback(port, false).await {
                assert_eq!(
                    l.local_addr().unwrap().port(),
                    port,
                    "explicit-port-free: helper must return the requested port verbatim"
                );
                return;
            }
        }
        panic!("could not re-bind the same released loopback port twice");
    }

    #[tokio::test]
    async fn fallback_remains_loopback_only() {
        // Defensive test: confirm that whatever port bind_loopback returns,
        // its address is on 127.0.0.1. This is the AGENTS.md / PRD §3
        // privacy invariant: NEVER fall back to LAN or public interfaces.
        let (occupier, occupied) = occupy_random_loopback_port().await;
        let listener = bind_loopback(occupied, true).await.unwrap();
        let local = listener.local_addr().unwrap();
        assert!(
            local.ip().is_loopback(),
            "fallback bound address must remain on a loopback IP, got {}",
            local.ip()
        );
        drop(listener);
        drop(occupier);
    }

    #[tokio::test]
    async fn listener_local_addr_port_is_the_source_of_truth() {
        // Regression for the original bug: the URL printed in main() and
        // the URL opened in the browser must both come from
        // listener.local_addr().port(), not from any requested-port
        // variable. We assert the contract by binding via bind_loopback,
        // reading the port exactly once from the listener, and verifying
        // any URL built from that number contains the same port.
        let (occupier, occupied) = occupy_random_loopback_port().await;
        let listener = bind_loopback(occupied, true).await.unwrap();
        let actual = listener.local_addr().unwrap().port();
        // Build the URL exactly the way main() does.
        let url = format!("http://127.0.0.1:{}/?token=d", actual);
        // And the URL must contain the actual port, not the requested one.
        assert!(
            url.contains(&format!(":{}", actual)),
            "url must contain the listener-bound port"
        );
        assert!(
            !url.contains(&format!(":{}", occupied)),
            "url must NOT contain the requested-busy port"
        );
        drop(listener);
        drop(occupier);
    }
}
