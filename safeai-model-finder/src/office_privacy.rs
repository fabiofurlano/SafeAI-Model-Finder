//! SafeAI Suite → SafeAI Office Privacy Filter — component installer.
//!
//! This module is deliberately separate from the Ollama model path
//! (`/api/pulls`, `OllamaProvider`, `llmfit-core`). The Office Privacy Filter
//! is **not** an Ollama model: it installs without Ollama running, without a
//! daemon, without a system service, and without touching Ollama's storage.
//! Keeping it in its own module makes that separation reviewable at a glance.
//!
//! ## Install flow
//!
//! detect platform + architecture → choose the exact approved artifact
//! contract → download into Office-owned staging → verify SHA-256 → verify
//! every required runtime file → atomically promote into Office-owned durable
//! storage → atomically write the completed manifest → UI reports Installed.
//!
//! A partial download or a partial extraction can never become an installed
//! component: staging is discarded on every failure and the manifest — the
//! only thing that defines "installed" — is written last.
//!
//! ## Storage
//!
//! The durable root is the SafeAI Office `userData` equivalent. The Office
//! `shell` app ships `productName: "SafeAI Office"`, so `app.getPath('userData')`
//! resolves to `%APPDATA%\SafeAI Office` on Windows,
//! `~/Library/Application Support/SafeAI Office` on macOS and
//! `${XDG_CONFIG_HOME:-~/.config}/SafeAI Office` on Linux. The component lives
//! in the `office-privacy` subdirectory of that root.
//!
//! SafeAI Desktop's own roots (`SafeAI/privacy-models`,
//! `SafeAI/privacy-runtime`) are never used or shared: Desktop and Office stay
//! separate products with separate storage, no symlinks and no shared folders.
//!
//! ## Artifact ownership
//!
//! Customer downloads must come from `fabiofurlano/safeai-office-runtime`.
//! Windows x64 is published there and reports [`PlatformSupport::Published`].
//! A target whose Office-owned asset is not published reports
//! `AwaitingOfficeArtifact` and refuses the customer download; a target with no
//! proven runtime at all reports `Unsupported`. A missing URL is never
//! substituted with a Desktop URL or a placeholder.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

// ── Component identity ────────────────────────────────────────────

/// Stable component identifier recorded in the manifest.
pub const COMPONENT_ID: &str = "safeai-office-privacy-filter";

/// Version of the component contract (not of Model Finder itself).
pub const COMPONENT_VERSION: &str = "1.0.0";

/// Manifest schema version.
pub const MANIFEST_SCHEMA: u32 = 1;

/// Manifest filename, relative to the Office privacy root.
pub const MANIFEST_FILE: &str = "manifest.json";

/// Directory name of the installed component, relative to the Office privacy root.
pub const COMPONENT_DIR: &str = "component";

/// Directory name of the Office privacy root inside the Office `userData` root.
pub const OFFICE_PRIVACY_DIR: &str = "office-privacy";

/// Office application directory name inside the platform `userData` root
/// (the Office `shell` app's `productName`).
pub const OFFICE_APP_DIR: &str = "SafeAI Office";

/// Test / power-user override for the durable root, mirroring the existing
/// `SAFEAI_MF_DATA_DIR` convention used for measurements.
pub const ENV_ROOT_OVERRIDE: &str = "SAFEAI_OFFICE_PRIVACY_DIR";

/// Optional override pointing at a mirror or offline copy of the *Office-owned*
/// runtime archive.
///
/// It is only honoured for a target whose archive digest is already pinned, and
/// the pinned digest is still enforced after download — so this can relocate the
/// download but can never substitute different content. It exists for an
/// internal mirror or an air-gapped staging copy of the identical archive, and
/// it is never a way to install an unverified runtime.
pub const ENV_RUNTIME_URL_OVERRIDE: &str = "SAFEAI_OFFICE_PRIVACY_RUNTIME_URL";

// ── Published Office-owned Windows x64 runtime artifact ───────────
//
// These are the real, public, anonymously fetchable values for the Office-owned
// Windows runtime. They belong to `fabiofurlano/safeai-office-runtime`, not to
// the SafeAI Desktop runtime repository, so SafeAI Office never depends on a
// Desktop asset. The tag is a standalone component release: it is deliberately
// NOT the SafeAI Office application updater release.

/// Published Office-owned component release tag.
pub const OFFICE_PRIVACY_RELEASE_TAG: &str = "office-privacy-v1.0.0";

/// Published Office-owned Windows x64 runtime asset filename.
pub const WINDOWS_ARTIFACT_NAME: &str =
    "safeai-office-privacy-runtime-windows-x64-v1.0.0.zip";

/// SHA-256 of the published Office-owned Windows x64 runtime archive.
pub const WINDOWS_ARTIFACT_SHA256: &str =
    "0bc2e0aa0177f2b82b3ae385f3adc1c65f515bac1730549840e118a34b08886b";

/// Public, anonymously fetchable download URL for that archive.
///
/// Broken across `concat!` pieces only to respect the line-length limit; the
/// resulting string is one unbroken URL with no whitespace in it.
pub const WINDOWS_ARTIFACT_URL: &str = concat!(
    "https://github.com/fabiofurlano/safeai-office-runtime/releases/download/",
    "office-privacy-v1.0.0/",
    "safeai-office-privacy-runtime-windows-x64-v1.0.0.zip",
);

// ── Prepared but not yet published Office-owned Linux x64 runtime artifact ──
//
// The Linux x64 runtime contract is proven (the four `pf-cli` files are the
// ones SafeAI Desktop actually ships on Linux) and its Office-owned archive has
// been built and hashed, so the digest below is real and final.
//
// The release asset is nevertheless **not published**, so this target has no
// `artifact_url` and stays [`PlatformSupport::AwaitingOfficeArtifact`]. The
// customer download is therefore still refused, and `resolved_url` only returns
// something for this target when a caller supplies the digest-gated mirror
// override ([`ENV_RUNTIME_URL_OVERRIDE`]) — which is what the local Linux proof
// uses, and which can still never install anything but this exact archive.
//
// Publishing is consequently a contained change: give the Linux arm of
// [`plan_for`] its `artifact_url` and flip its `support` to `Published`. The
// digest does not change, because it is the digest of the exact archive that
// will be published.

/// Prepared Office-owned Linux x64 runtime asset filename.
pub const LINUX_ARTIFACT_NAME: &str =
    "safeai-office-privacy-runtime-linux-x64-v1.0.0.tar.gz";

/// SHA-256 of the prepared Office-owned Linux x64 runtime archive
/// (`safeai-office-privacy-runtime-linux-x64-v1.0.0.tar.gz`, 748497 bytes).
pub const LINUX_ARTIFACT_SHA256: &str =
    "95d4b9c6d63eb87f3a85b1de338b50474870edae81172ef08087f817bdb34ce9";

// ── Proven upstream privacy contracts (SafeAI Desktop, read-only) ──
//
// The multilingual privacy filter model and the Windows `pf-cli` runtime are
// the currently proven SafeAI Desktop artifacts. Only the *contract* is reused
// here (filenames, digests, archive layout); no Desktop path is read or written.

/// Hugging Face repository that publishes the proven privacy filter model.
pub const MODEL_REPO: &str = "LocalAI-io/privacy-filter-multilingual-GGUF";

/// Proven multilingual GGUF filename.
pub const MODEL_FILE: &str = "privacy-filter-multilingual-q8.gguf";

/// Proven SHA-256 of [`MODEL_FILE`].
pub const MODEL_SHA256: &str =
    "968135172ba8202374b4c3bd7d353e100c8fc574035da793fa4d13ca441319b7";

/// Read-only provenance label for the platform contracts below.
pub const CONTRACT_PROVENANCE: &str = "SafeAI Desktop runtime-manager.js (safeai-runtime-v1.0.0)";

/// Proven layout of the Windows runtime inside its archive, relative to the
/// component directory. Mirrors Desktop's extraction root
/// (`{runtimeCache}/privacy-filter.cpp`) plus the archive's own
/// `build/safeai-release/bin/Release/` prefix.
const WINDOWS_RUNTIME_DIR: &str =
    "runtime/privacy-filter.cpp/build/safeai-release/bin/Release/";

/// Runtime directory for the Linux bundled layout, relative to the component
/// directory. Mirrors Desktop's `resources/privacy-runtime` layout.
const LINUX_RUNTIME_DIR: &str = "runtime/";

/// The twelve proven Windows `pf-cli` files.
const WINDOWS_PROVEN_RUNTIME_FILES: &[&str] = &[
    "pf-cli.exe",
    "ggml.dll",
    "ggml-base.dll",
    "ggml-cpu-x64.dll",
    "ggml-cpu-sse42.dll",
    "ggml-cpu-haswell.dll",
    "ggml-cpu-icelake.dll",
    "ggml-cpu-alderlake.dll",
    "ggml-cpu-cannonlake.dll",
    "ggml-cpu-cascadelake.dll",
    "ggml-cpu-skylakex.dll",
    "ggml-cpu-sandybridge.dll",
];

/// Windows MSVC C++ runtime files the proven archive does **not** contain.
///
/// Every binary in `safeai-windows-privacy-runtime.zip` imports
/// `MSVCP140.dll`, `VCRUNTIME140.dll` and `VCRUNTIME140_1.dll`, and the two
/// GGML base libraries additionally import `VCOMP140.DLL` (OpenMP). The
/// archive ships none of them — Desktop satisfies the prerequisite by running
/// the Visual C++ redistributable installer, which is the historical BUG-0009
/// shape (a clean Windows machine failing on missing native runtime DLLs).
///
/// SafeAI Office has no installer of its own on this path, so the smallest
/// equivalent self-contained approach is application-local deployment: the
/// Office-owned archive must carry these four DLLs next to `pf-cli.exe`,
/// exactly as Microsoft documents for app-local VC++ deployment. That keeps a
/// clean machine working with no separate redistributable step.
///
/// The `api-ms-win-crt-*` forwarders and `KERNEL32.dll` in the same import
/// tables are operating-system components (the Universal CRT ships with
/// Windows 10 and later) and are deliberately not part of the contract.
const WINDOWS_VC_RUNTIME_FILES: &[&str] = &[
    "MSVCP140.dll",
    "VCRUNTIME140.dll",
    "VCRUNTIME140_1.dll",
    "VCOMP140.DLL",
];

/// Proven Linux bundled runtime files (SafeAI Desktop `resources/privacy-runtime`).
const LINUX_PROVEN_RUNTIME_FILES: &[&str] = &[
    "pf-cli",
    "ggml/src/libggml.so.0",
    "ggml/src/libggml-base.so.0",
    "bin/libggml-cpu-x64.so",
];

/// Component-relative Windows runtime files: the twelve proven files plus the
/// four app-local VC++ runtime DLLs that make the package self-contained.
static WINDOWS_REQUIRED_FILES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    WINDOWS_PROVEN_RUNTIME_FILES
        .iter()
        .chain(WINDOWS_VC_RUNTIME_FILES.iter())
        .map(|name| format!("{WINDOWS_RUNTIME_DIR}{name}"))
        .map(|path| &*Box::leak(path.into_boxed_str()))
        .collect()
});

/// Component-relative Linux runtime files.
static LINUX_REQUIRED_FILES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    LINUX_PROVEN_RUNTIME_FILES
        .iter()
        .map(|name| format!("{LINUX_RUNTIME_DIR}{name}"))
        .map(|path| &*Box::leak(path.into_boxed_str()))
        .collect()
});

/// Component-relative path of the `pf-cli` binary on Windows.
pub const WINDOWS_RELATIVE_BINARY: &str =
    "runtime/privacy-filter.cpp/build/safeai-release/bin/Release/pf-cli.exe";

/// Component-relative path of the `pf-cli` binary on Linux.
pub const LINUX_RELATIVE_BINARY: &str = "runtime/pf-cli";

// ── Target detection ──────────────────────────────────────────────

/// Operating-system lane of the Office Privacy component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfficeOs {
    Windows,
    Linux,
    Macos,
    Other,
}

impl OfficeOs {
    /// Detect the current lane from the compiled target.
    pub fn current() -> Self {
        match std::env::consts::OS {
            "windows" => OfficeOs::Windows,
            "linux" => OfficeOs::Linux,
            "macos" => OfficeOs::Macos,
            _ => OfficeOs::Other,
        }
    }

    /// Stable manifest / API string.
    pub fn as_str(self) -> &'static str {
        match self {
            OfficeOs::Windows => "windows",
            OfficeOs::Linux => "linux",
            OfficeOs::Macos => "macos",
            OfficeOs::Other => "other",
        }
    }
}

/// CPU architecture lane of the Office Privacy component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfficeArch {
    X86_64,
    Aarch64,
    Other,
}

impl OfficeArch {
    /// Detect the current lane from the compiled target.
    pub fn current() -> Self {
        match std::env::consts::ARCH {
            "x86_64" => OfficeArch::X86_64,
            "aarch64" => OfficeArch::Aarch64,
            _ => OfficeArch::Other,
        }
    }

    /// Stable manifest / API string.
    pub fn as_str(self) -> &'static str {
        match self {
            OfficeArch::X86_64 => "x86_64",
            OfficeArch::Aarch64 => "aarch64",
            OfficeArch::Other => "other",
        }
    }
}

/// Resolved platform + architecture for artifact selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfficeTarget {
    pub os: OfficeOs,
    pub arch: OfficeArch,
}

impl OfficeTarget {
    /// Detect the running target.
    pub fn detect() -> Self {
        Self {
            os: OfficeOs::current(),
            arch: OfficeArch::current(),
        }
    }

    /// Explicit constructor for a specific lane.
    ///
    /// The running app always uses [`OfficeTarget::detect`]; this is the entry
    /// point tests and any future explicit-lane caller use, so it is exercised
    /// by the test suite rather than by the request path.
    #[allow(dead_code)]
    pub fn new(os: OfficeOs, arch: OfficeArch) -> Self {
        Self { os, arch }
    }
}

// ── Support level + artifact plan ─────────────────────────────────

/// How far the Office Privacy component can go on a given target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformSupport {
    /// The Office-owned customer artifact is published and verified, so the
    /// customer download is enabled for this target.
    Published,
    /// The runtime contract is proven for this target, but the Office-owned
    /// customer artifact has not been published yet. The installer structure
    /// is fully implemented; the customer download is refused.
    AwaitingOfficeArtifact,
    /// The native privacy runtime itself has never been proven for this
    /// target, so no artifact can be requested at all.
    Unsupported,
}

impl PlatformSupport {
    /// Stable API / UI string.
    pub fn as_str(self) -> &'static str {
        match self {
            PlatformSupport::Published => "published",
            PlatformSupport::AwaitingOfficeArtifact => "awaiting_office_artifact",
            PlatformSupport::Unsupported => "unsupported",
        }
    }
}

/// Archive container format of a release asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    TarGz,
}

impl ArchiveFormat {
    /// Stable API / UI string.
    pub fn as_str(self) -> &'static str {
        match self {
            ArchiveFormat::Zip => "zip",
            ArchiveFormat::TarGz => "tar.gz",
        }
    }
}

/// The exact approved artifact contract for one target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfficePrivacyPlan {
    pub target: OfficeTarget,
    pub support: PlatformSupport,
    /// Component-level detail for the UI and for logs.
    pub support_detail: &'static str,
    /// The Office-owned release asset that must be published for this target.
    pub artifact_name: &'static str,
    /// Archive format of that asset.
    pub artifact_format: ArchiveFormat,
    /// SHA-256 of the Office-owned archive. `None` until published.
    pub artifact_sha256: Option<&'static str>,
    /// Published download URL. `None` until the asset exists.
    pub artifact_url: Option<&'static str>,
    /// Where the archive root maps inside the component directory.
    pub archive_prefix: &'static str,
    /// Every file that must exist (non-empty) inside the component directory.
    pub required_runtime_files: &'static [&'static str],
    /// Provenance of the contract, for audit.
    pub provenance: &'static str,
}

/// Look up the approved artifact contract for a target.
///
/// Windows x64 is [`PlatformSupport::Published`]: its Office-owned archive is
/// live on `fabiofurlano/safeai-office-runtime` and its URL and digest are the
/// pinned constants above, so the customer download is enabled.
///
/// Linux reports `AwaitingOfficeArtifact` — the contract is proven and its
/// Office-owned archive is prepared and digest-pinned, but the release asset is
/// not published, so there is no customer download. macOS reports
/// `Unsupported`: no macOS privacy runtime has ever been built or proven.
/// Enabling either is a matter of filling in `artifact_url` (and, for Linux,
/// flipping `support`) here; the Linux digest is already pinned.
pub fn plan_for(target: OfficeTarget) -> OfficePrivacyPlan {
    match (target.os, target.arch) {
        (OfficeOs::Windows, OfficeArch::X86_64) => OfficePrivacyPlan {
            target,
            support: PlatformSupport::Published,
            support_detail: "Windows x64 runtime is published by SafeAI Office and ready to install.",
            artifact_name: WINDOWS_ARTIFACT_NAME,
            artifact_format: ArchiveFormat::Zip,
            artifact_sha256: Some(WINDOWS_ARTIFACT_SHA256),
            artifact_url: Some(WINDOWS_ARTIFACT_URL),
            archive_prefix: WINDOWS_RUNTIME_DIR,
            required_runtime_files: WINDOWS_REQUIRED_FILES.as_slice(),
            provenance: CONTRACT_PROVENANCE,
        },
        (OfficeOs::Linux, OfficeArch::X86_64) => OfficePrivacyPlan {
            target,
            support: PlatformSupport::AwaitingOfficeArtifact,
            support_detail: "Linux x64 runtime is proven and its Office-owned archive is prepared \
                             and digest-pinned, but the release asset is not published yet.",
            artifact_name: LINUX_ARTIFACT_NAME,
            artifact_format: ArchiveFormat::TarGz,
            artifact_sha256: Some(LINUX_ARTIFACT_SHA256),
            artifact_url: None,
            archive_prefix: LINUX_RUNTIME_DIR,
            required_runtime_files: LINUX_REQUIRED_FILES.as_slice(),
            provenance: CONTRACT_PROVENANCE,
        },
        (OfficeOs::Macos, OfficeArch::X86_64) => OfficePrivacyPlan {
            target,
            support: PlatformSupport::Unsupported,
            support_detail: "No macOS Intel privacy runtime has ever been built or proven, so \
                             there is no artifact contract to install.",
            artifact_name: "safeai-office-privacy-runtime-macos-x64-v1.0.0.tar.gz",
            artifact_format: ArchiveFormat::TarGz,
            artifact_sha256: None,
            artifact_url: None,
            archive_prefix: LINUX_RUNTIME_DIR,
            required_runtime_files: &[],
            provenance: CONTRACT_PROVENANCE,
        },
        (OfficeOs::Macos, OfficeArch::Aarch64) => OfficePrivacyPlan {
            target,
            support: PlatformSupport::Unsupported,
            support_detail: "No macOS Apple Silicon privacy runtime has ever been built or \
                             proven, so there is no artifact contract to install.",
            artifact_name: "safeai-office-privacy-runtime-macos-arm64-v1.0.0.tar.gz",
            artifact_format: ArchiveFormat::TarGz,
            artifact_sha256: None,
            artifact_url: None,
            archive_prefix: LINUX_RUNTIME_DIR,
            required_runtime_files: &[],
            provenance: CONTRACT_PROVENANCE,
        },
        _ => OfficePrivacyPlan {
            target,
            support: PlatformSupport::Unsupported,
            support_detail: "This platform and architecture combination has no proven privacy \
                             runtime artifact.",
            artifact_name: "safeai-office-privacy-runtime-unsupported",
            artifact_format: ArchiveFormat::TarGz,
            artifact_sha256: None,
            artifact_url: None,
            archive_prefix: LINUX_RUNTIME_DIR,
            required_runtime_files: &[],
            provenance: CONTRACT_PROVENANCE,
        },
    }
}

/// Component-relative path of the `pf-cli` binary for a plan's target.
pub fn relative_binary_for(plan: &OfficePrivacyPlan) -> Option<&'static str> {
    match plan.target.os {
        OfficeOs::Windows => Some(WINDOWS_RELATIVE_BINARY),
        OfficeOs::Linux => Some(LINUX_RELATIVE_BINARY),
        _ => None,
    }
}

/// Component-relative path of the privacy filter model.
pub fn relative_model_path() -> String {
    format!("model/{MODEL_REPO}/{MODEL_FILE}")
}

// ── Storage root ──────────────────────────────────────────────────

/// Resolve the Office-owned durable privacy root.
///
/// Returns `None` on platforms with no Office `userData` equivalent, which is
/// also the platform set with no runtime contract.
pub fn office_privacy_root() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var(ENV_ROOT_OVERRIDE)
        && !dir.is_empty()
    {
        return Some(PathBuf::from(dir).join(OFFICE_PRIVACY_DIR));
    }
    office_userdata_root().map(|root| root.join(OFFICE_PRIVACY_DIR))
}

/// Resolve the Office `app.getPath('userData')` root for the current platform.
///
/// This follows Electron's own resolution: `%APPDATA%` on Windows,
/// `~/Library/Application Support` on macOS, and `$XDG_CONFIG_HOME` (falling
/// back to `~/.config`) on Linux, each with the Office app directory appended.
pub fn office_userdata_root() -> Option<PathBuf> {
    let app_data = match OfficeOs::current() {
        OfficeOs::Windows => std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(|h| PathBuf::from(h).join("AppData/Roaming"))
        })?,
        OfficeOs::Macos => {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))?
        }
        OfficeOs::Linux => std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?,
        OfficeOs::Other => return None,
    };
    Some(app_data.join(OFFICE_APP_DIR))
}

/// Durable, Office-owned component directory for a root.
pub fn component_dir(root: &Path) -> PathBuf {
    root.join(COMPONENT_DIR)
}

/// Durable manifest path for a root.
pub fn manifest_path(root: &Path) -> PathBuf {
    root.join(MANIFEST_FILE)
}

/// Staging directory used while downloading, extracting and verifying.
///
/// It sits inside the Office privacy root so promotion is a same-filesystem
/// rename (atomic), never a cross-device copy.
pub fn staging_dir(root: &Path, token: &str) -> PathBuf {
    root.join(format!(".staging-{token}"))
}

// ── Errors ────────────────────────────────────────────────────────

/// Every way an Office Privacy install can fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    UnsupportedPlatform,
    ArtifactNotPublished { artifact_name: &'static str },
    Download(String),
    ChecksumMismatch { expected: String, actual: String },
    Extraction(String),
    MissingRuntimeFiles(Vec<String>),
    Io(String),
    Promote(String),
}

impl InstallError {
    /// Stable machine code returned by the HTTP layer.
    pub fn code(&self) -> &'static str {
        match self {
            InstallError::UnsupportedPlatform => "unsupported_platform",
            InstallError::ArtifactNotPublished { .. } => "artifact_not_published",
            InstallError::Download(_) => "download_failed",
            InstallError::ChecksumMismatch { .. } => "checksum_mismatch",
            InstallError::Extraction(_) => "extraction_failed",
            InstallError::MissingRuntimeFiles(_) => "missing_runtime_files",
            InstallError::Io(_) => "io_error",
            InstallError::Promote(_) => "promote_failed",
        }
    }

    /// Human-readable, path-free message safe to show in the UI.
    pub fn user_message(&self) -> String {
        match self {
            InstallError::UnsupportedPlatform => {
                "The SafeAI Office Privacy Filter is not available on this platform yet.".into()
            }
            InstallError::ArtifactNotPublished { artifact_name } => format!(
                "The SafeAI Office Privacy Filter runtime is not published yet. \
                 Waiting for the SafeAI Office release asset: {artifact_name}"
            ),
            InstallError::Download(e) => format!("The download did not complete: {e}"),
            InstallError::ChecksumMismatch { .. } => {
                "The downloaded runtime file failed its integrity check, so it was discarded."
                    .into()
            }
            InstallError::Extraction(e) => {
                format!("The runtime archive could not be unpacked: {e}")
            }
            InstallError::MissingRuntimeFiles(files) => format!(
                "The runtime archive is incomplete ({} required file(s) missing), so nothing \
                 was installed.",
                files.len()
            ),
            InstallError::Io(e) => format!("A local file operation failed: {e}"),
            InstallError::Promote(e) => {
                format!("The verified component could not be activated: {e}")
            }
        }
    }
}

// ── Manifest ──────────────────────────────────────────────────────

/// Model section of the completed manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestModel {
    pub relative_path: String,
    pub sha256: String,
}

/// Runtime section of the completed manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestRuntime {
    pub relative_binary: String,
    pub package_sha256: String,
    pub required_files: Vec<String>,
}

/// The completed-installation manifest.
///
/// Written exactly once, after a verified installation, and never in an
/// installing, failed or partial state. Every path is relative to the
/// component directory: absolute paths and download URLs are deliberately
/// absent so the manifest stays portable and cannot leak a machine layout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfficePrivacyManifest {
    pub schema: u32,
    pub component_id: String,
    pub component_version: String,
    pub platform: String,
    pub architecture: String,
    pub installed_at: String,
    pub model: ManifestModel,
    pub runtime: ManifestRuntime,
}

impl OfficePrivacyManifest {
    /// Build the manifest for a completed installation.
    ///
    /// `package_sha256` is the verified SHA-256 of the runtime archive that
    /// produced this component.
    pub fn build(
        plan: &OfficePrivacyPlan,
        package_sha256: &str,
        installed_at: String,
    ) -> Result<Self, InstallError> {
        let Some(relative_binary) = relative_binary_for(plan) else {
            return Err(InstallError::UnsupportedPlatform);
        };
        Ok(Self {
            schema: MANIFEST_SCHEMA,
            component_id: COMPONENT_ID.to_string(),
            component_version: COMPONENT_VERSION.to_string(),
            platform: plan.target.os.as_str().to_string(),
            architecture: plan.target.arch.as_str().to_string(),
            installed_at,
            model: ManifestModel {
                relative_path: relative_model_path(),
                sha256: MODEL_SHA256.to_string(),
            },
            runtime: ManifestRuntime {
                relative_binary: relative_binary.to_string(),
                package_sha256: package_sha256.to_string(),
                required_files: plan
                    .required_runtime_files
                    .iter()
                    .map(|f| (*f).to_string())
                    .collect(),
            },
        })
    }

    /// True when every recorded path is relative and traversal-free.
    ///
    /// This is the invariant that keeps the manifest portable and stops a
    /// crafted manifest from pointing the component at an arbitrary location.
    pub fn paths_are_relative(&self) -> bool {
        let ok = |p: &str| {
            !p.is_empty()
                && !p.starts_with('/')
                && !p.starts_with('\\')
                && !p.contains(':')
                && !p
                    .split(['/', '\\'])
                    .any(|seg| seg == ".." || seg.is_empty())
        };
        ok(&self.model.relative_path)
            && ok(&self.runtime.relative_binary)
            && self.runtime.required_files.iter().all(|f| ok(f))
    }
}

/// Read the completed manifest, if one exists and is well-formed.
pub fn read_manifest(root: &Path) -> Option<OfficePrivacyManifest> {
    let raw = std::fs::read_to_string(manifest_path(root)).ok()?;
    let manifest: OfficePrivacyManifest = serde_json::from_str(&raw).ok()?;
    if manifest.schema != MANIFEST_SCHEMA
        || manifest.component_id != COMPONENT_ID
        || !manifest.paths_are_relative()
    {
        return None;
    }
    Some(manifest)
}

/// Write the manifest atomically (temp file in the same directory + rename).
///
/// A crash or a failed write therefore leaves either the previous manifest or
/// no manifest at all — never a truncated one.
pub fn write_manifest(root: &Path, manifest: &OfficePrivacyManifest) -> Result<(), InstallError> {
    if !manifest.paths_are_relative() {
        return Err(InstallError::Io(
            "refusing to write a manifest containing non-relative paths".to_string(),
        ));
    }
    std::fs::create_dir_all(root).map_err(|e| InstallError::Io(e.to_string()))?;
    let json =
        serde_json::to_string_pretty(manifest).map_err(|e| InstallError::Io(e.to_string()))?;
    let target = manifest_path(root);
    let temp = root.join(format!(".{MANIFEST_FILE}.tmp-{}", std::process::id()));
    std::fs::write(&temp, json).map_err(|e| InstallError::Io(e.to_string()))?;
    std::fs::rename(&temp, &target).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        InstallError::Io(e.to_string())
    })
}

// ── Hashing ───────────────────────────────────────────────────────

/// Lowercase hex SHA-256 of a byte slice.
///
/// The install path hashes files by streaming ([`sha256_file`]) so a
/// multi-gigabyte model never has to be held in memory; this in-memory form is
/// the small-input counterpart used to pin and compare expected digests.
#[allow(dead_code)]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Lowercase hex SHA-256 of a file, streamed so large models stay cheap.
pub fn sha256_file(path: &Path) -> Result<String, InstallError> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path).map_err(|e| InstallError::Io(e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 128 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| InstallError::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Compare two hex digests case-insensitively, rejecting length mismatches.
pub fn digests_match(expected: &str, actual: &str) -> bool {
    expected.len() == actual.len() && expected.eq_ignore_ascii_case(actual)
}

/// Verify an already-staged payload against a pinned archive digest.
///
/// [`download_archive`] enforces the digest while streaming so a bad download is
/// discarded immediately; this is the standalone check for a file that already
/// exists on disk (only reachable once an Office-owned artifact is published,
/// plus the checksum tests).
#[allow(dead_code)]
pub fn verify_archive_digest(staged_file: &Path, expected_sha256: &str) -> Result<(), InstallError> {
    let actual = sha256_file(staged_file)?;
    if digests_match(expected_sha256, &actual) {
        Ok(())
    } else {
        Err(InstallError::ChecksumMismatch {
            expected: expected_sha256.to_string(),
            actual,
        })
    }
}

// ── Required-file verification ────────────────────────────────────

/// Return every required runtime file that is missing or empty under `base`.
///
/// `base` is the staged component directory — the same layout the durable
/// component directory will have.
pub fn missing_required_files(base: &Path, plan: &OfficePrivacyPlan) -> Vec<String> {
    plan.required_runtime_files
        .iter()
        .filter(|rel| match std::fs::metadata(base.join(rel)) {
            Ok(m) => !m.is_file() || m.len() == 0,
            Err(_) => true,
        })
        .map(|rel| (*rel).to_string())
        .collect()
}

// ── Atomic promotion ──────────────────────────────────────────────

/// Promote a fully verified staging directory into the durable component slot.
///
/// Same-filesystem rename with a short-lived backup of any previous component,
/// so a failure restores the previous installation instead of leaving a
/// half-written one.
pub fn promote_staged_component(staging: &Path, root: &Path) -> Result<(), InstallError> {
    let final_dir = component_dir(root);
    std::fs::create_dir_all(root).map_err(|e| InstallError::Promote(e.to_string()))?;

    let backup = root.join(format!(".{COMPONENT_DIR}.previous-{}", std::process::id()));
    if backup.exists() {
        std::fs::remove_dir_all(&backup).map_err(|e| InstallError::Promote(e.to_string()))?;
    }

    let had_previous = final_dir.exists();
    if had_previous {
        std::fs::rename(&final_dir, &backup).map_err(|e| InstallError::Promote(e.to_string()))?;
    }

    match std::fs::rename(staging, &final_dir) {
        Ok(()) => {
            if had_previous {
                let _ = std::fs::remove_dir_all(&backup);
            }
            Ok(())
        }
        Err(e) => {
            if had_previous {
                let _ = std::fs::rename(&backup, &final_dir);
            }
            Err(InstallError::Promote(e.to_string()))
        }
    }
}

// ── Finalize ──────────────────────────────────────────────────────

/// Verify a staged component and, only if everything checks out, promote it
/// and write the completed manifest.
///
/// This is the single point where "installed" becomes true. It performs no
/// network access, which is what makes the whole contract testable.
pub fn finalize_install(
    root: &Path,
    staging: &Path,
    plan: &OfficePrivacyPlan,
    archive_sha256: &str,
    installed_at: String,
) -> Result<OfficePrivacyManifest, InstallError> {
    finalize_install_with_model_digest(
        root,
        staging,
        plan,
        archive_sha256,
        MODEL_SHA256,
        installed_at,
    )
}

/// [`finalize_install`] with an explicit expected model digest.
///
/// Production always goes through [`finalize_install`], which pins the proven
/// multilingual GGUF digest. The injected form exists so the full success path
/// can be exercised deterministically without shipping a multi-gigabyte
/// fixture.
pub fn finalize_install_with_model_digest(
    root: &Path,
    staging: &Path,
    plan: &OfficePrivacyPlan,
    archive_sha256: &str,
    expected_model_sha256: &str,
    installed_at: String,
) -> Result<OfficePrivacyManifest, InstallError> {
    if plan.support == PlatformSupport::Unsupported {
        return Err(InstallError::UnsupportedPlatform);
    }

    // 1. Every required runtime file must be present and non-empty.
    let missing = missing_required_files(staging, plan);
    if !missing.is_empty() {
        return Err(InstallError::MissingRuntimeFiles(missing));
    }

    // 2. The model must be present and match its pinned digest.
    let actual_model = sha256_file(&staging.join(relative_model_path()))?;
    if !digests_match(expected_model_sha256, &actual_model) {
        return Err(InstallError::ChecksumMismatch {
            expected: expected_model_sha256.to_string(),
            actual: actual_model,
        });
    }

    // 3. Build the manifest before touching durable state, so a component whose
    //    manifest cannot be represented is never left behind.
    let manifest = OfficePrivacyManifest::build(plan, archive_sha256, installed_at)?;

    // 4. Promote, then write the manifest. The manifest is the only thing that
    //    defines "installed", so if it cannot be written the component that was
    //    just promoted is removed again.
    promote_staged_component(staging, root)?;
    if let Err(e) = write_manifest(root, &manifest) {
        let _ = std::fs::remove_dir_all(component_dir(root));
        return Err(e);
    }
    Ok(manifest)
}

// ── Installed-state assessment ────────────────────────────────────

/// User-visible state of the Office Privacy component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfficePrivacyState {
    NotInstalled,
    Installed,
    Unsupported,
    AwaitingArtifact,
}

impl OfficePrivacyState {
    /// Stable API / UI string.
    pub fn as_str(self) -> &'static str {
        match self {
            OfficePrivacyState::NotInstalled => "not_installed",
            OfficePrivacyState::Installed => "installed",
            OfficePrivacyState::Unsupported => "unsupported",
            OfficePrivacyState::AwaitingArtifact => "awaiting_artifact",
        }
    }
}

/// Assess the durable state for a target.
///
/// `Installed` requires a well-formed manifest whose every recorded file exists
/// on disk — a leftover partial directory never counts as installed.
pub fn assess(root: Option<&Path>, plan: &OfficePrivacyPlan) -> (OfficePrivacyState, Vec<String>) {
    if plan.support == PlatformSupport::Unsupported {
        return (OfficePrivacyState::Unsupported, Vec::new());
    }
    let Some(root) = root else {
        return (OfficePrivacyState::Unsupported, Vec::new());
    };
    let Some(manifest) = read_manifest(root) else {
        return (OfficePrivacyState::NotInstalled, Vec::new());
    };
    let base = component_dir(root);
    let mut missing: Vec<String> = manifest
        .runtime
        .required_files
        .iter()
        .filter(|rel| !base.join(rel).is_file())
        .cloned()
        .collect();
    if !base.join(&manifest.model.relative_path).is_file() {
        missing.push(manifest.model.relative_path.clone());
    }
    if missing.is_empty() {
        (OfficePrivacyState::Installed, missing)
    } else {
        (OfficePrivacyState::NotInstalled, missing)
    }
}

/// [`assess`], with the "waiting for an Office release asset" case surfaced.
///
/// A target whose runtime contract is proven but whose Office-owned archive is
/// not published yet is not merely "not installed": the UI must say it is
/// waiting on a release asset rather than offer a button that cannot work.
pub fn effective_state(
    root: Option<&Path>,
    plan: &OfficePrivacyPlan,
) -> (OfficePrivacyState, Vec<String>) {
    let (state, missing) = assess(root, plan);
    if state == OfficePrivacyState::NotInstalled
        && plan.support == PlatformSupport::AwaitingOfficeArtifact
        && resolved_url(plan).is_none()
    {
        return (OfficePrivacyState::AwaitingArtifact, missing);
    }
    (state, missing)
}

/// Public, anonymously fetchable source of the proven privacy filter model.
pub fn model_url() -> String {
    format!("https://huggingface.co/{MODEL_REPO}/resolve/main/{MODEL_FILE}")
}

/// Where the runtime archive's own paths map inside a staging directory.
pub fn archive_extract_root(base: &Path, plan: &OfficePrivacyPlan) -> PathBuf {
    base.join(plan.archive_prefix.trim_end_matches('/'))
}

// ── Download + extraction ─────────────────────────────────────────

/// Download the runtime archive into staging, streaming SHA-256 as it goes.
///
/// Returns the verified digest of the archive. The staged file is removed on
/// any failure, so a partial download can never be mistaken for a good one.
pub fn download_archive(
    url: &str,
    destination: &Path,
    expected_sha256: Option<&str>,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<String, InstallError> {
    use sha2::{Digest, Sha256};

    let response = ureq::get(url)
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(1800)))
        .build()
        .call()
        .map_err(|e| InstallError::Download(e.to_string()))?;

    let total = response.body().content_length();
    let mut reader = response.into_body().into_reader();
    let mut file =
        std::fs::File::create(destination).map_err(|e| InstallError::Download(e.to_string()))?;

    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 128 * 1024];
    let mut last_report = std::time::Instant::now();

    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                drop(file);
                let _ = std::fs::remove_file(destination);
                return Err(InstallError::Download(e.to_string()));
            }
        };
        if let Err(e) = std::io::Write::write_all(&mut file, &buf[..n]) {
            drop(file);
            let _ = std::fs::remove_file(destination);
            return Err(InstallError::Download(e.to_string()));
        }
        hasher.update(&buf[..n]);
        downloaded += n as u64;
        if last_report.elapsed() >= std::time::Duration::from_millis(250) {
            on_progress(downloaded, total);
            last_report = std::time::Instant::now();
        }
    }
    if let Err(e) = std::io::Write::flush(&mut file) {
        drop(file);
        let _ = std::fs::remove_file(destination);
        return Err(InstallError::Download(e.to_string()));
    }
    drop(file);
    on_progress(downloaded, total);

    let actual = hex::encode(hasher.finalize());
    if let Some(expected) = expected_sha256
        && !digests_match(expected, &actual)
    {
        let _ = std::fs::remove_file(destination);
        return Err(InstallError::ChecksumMismatch {
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(actual)
}

/// Quote a path as a PowerShell single-quoted literal string.
///
/// The only escape inside a PowerShell single-quoted string is a doubled
/// quote (`'` → `''`), so doubling is both necessary and sufficient to stop a
/// path from terminating the literal and being parsed as PowerShell syntax.
/// Without this, a directory name containing an apostrophe — reachable through
/// the durable-root environment override — would break out of the quoting.
fn powershell_literal(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

/// Extract an archive using the operating system's own tooling.
///
/// No archive crate is added: on Windows this uses the same `Expand-Archive`
/// mechanism the proven SafeAI Desktop flow uses, and on Linux/macOS it uses
/// `unzip` / `tar`. A missing tool is a clear, reported failure rather than a
/// silent partial extraction.
///
/// Only the Windows path needs a command string, and both of its paths are
/// escaped literals — never interpolated raw. Every other path passes
/// arguments directly to the child process, so no shell is involved.
pub fn extract_archive(
    archive: &Path,
    format: ArchiveFormat,
    destination: &Path,
) -> Result<(), InstallError> {
    std::fs::create_dir_all(destination).map_err(|e| InstallError::Extraction(e.to_string()))?;

    let output = match format {
        ArchiveFormat::Zip => {
            if cfg!(windows) {
                let script = format!(
                    "Expand-Archive -LiteralPath {} -DestinationPath {} -Force",
                    powershell_literal(archive),
                    powershell_literal(destination),
                );
                std::process::Command::new("powershell")
                    .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                    .output()
            } else {
                std::process::Command::new("unzip")
                    .args(["-qq", "-o"])
                    .arg(archive)
                    .arg("-d")
                    .arg(destination)
                    .output()
            }
        }
        ArchiveFormat::TarGz => std::process::Command::new("tar")
            .args(["-xzf"])
            .arg(archive)
            .arg("-C")
            .arg(destination)
            .output(),
    };

    let output = output.map_err(|e| {
        InstallError::Extraction(format!(
            "no archive tool available for {}: {e}",
            format.as_str()
        ))
    })?;
    if !output.status.success() {
        return Err(InstallError::Extraction(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(())
}

/// Which archive URL to use for a plan.
///
/// A mirror override wins, but only for a target that already has a pinned
/// digest — and [`verify_archive_digest`] still runs on whatever is downloaded,
/// so relocating the download can never change what gets installed. Without an
/// override, the published Office-owned URL is used. Without either, there is
/// no URL and the install is refused.
pub fn resolved_url(plan: &OfficePrivacyPlan) -> Option<String> {
    if plan.artifact_sha256.is_some()
        && let Ok(url) = std::env::var(ENV_RUNTIME_URL_OVERRIDE)
        && !url.is_empty()
    {
        return Some(url);
    }
    plan.artifact_url.map(str::to_string)
}

/// Refuse a customer download that has no published Office-owned artifact.
pub fn require_published_artifact(plan: &OfficePrivacyPlan) -> Result<(), InstallError> {
    if plan.support == PlatformSupport::Unsupported {
        return Err(InstallError::UnsupportedPlatform);
    }
    // Fail closed: any supported target without a resolved URL refuses the
    // download, whatever its recorded support level says. A `Published` plan
    // that somehow lost its URL must not silently install from nowhere.
    if resolved_url(plan).is_none() {
        return Err(InstallError::ArtifactNotPublished {
            artifact_name: plan.artifact_name,
        });
    }
    Ok(())
}

// ── UTC timestamp ─────────────────────────────────────────────────

/// Format a `SystemTime` as an RFC 3339 UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`).
///
/// Implemented locally to avoid adding a date-time dependency.
pub fn format_rfc3339_utc(time: std::time::SystemTime) -> String {
    let secs = time
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    // Howard Hinnant's civil-from-days algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    if m <= 2 {
        y += 1;
    }
    format!("{y:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Unix timestamp at second precision, for the manifest's `installed_at`.
pub fn now_rfc3339() -> String {
    format_rfc3339_utc(std::time::SystemTime::now())
}

/// Crate-wide lock for every test that mutates the Office privacy environment.
///
/// `cargo test` runs tests in parallel threads of one binary, so the Office
/// platform tests and the API tests must serialise against each other rather
/// than each holding a private lock.
#[cfg(test)]
pub(crate) fn test_env_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "smf-office-privacy-{}-{}-{label}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Build a fully valid staged component: every required runtime file plus a
    /// model whose bytes are returned so the caller can pin its digest.
    fn stage_component(root: &Path, plan: &OfficePrivacyPlan, token: &str, model: &[u8]) -> PathBuf {
        let staging = staging_dir(root, token);
        for rel in plan.required_runtime_files {
            let path = staging.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"stub-runtime-bytes").unwrap();
        }
        let model_path = staging.join(relative_model_path());
        std::fs::create_dir_all(model_path.parent().unwrap()).unwrap();
        std::fs::write(&model_path, model).unwrap();
        staging
    }

    /// Windows plan helper.
    fn windows_plan() -> OfficePrivacyPlan {
        plan_for(OfficeTarget::new(OfficeOs::Windows, OfficeArch::X86_64))
    }

    /// Linux plan helper.
    fn linux_plan() -> OfficePrivacyPlan {
        plan_for(OfficeTarget::new(OfficeOs::Linux, OfficeArch::X86_64))
    }

    // ── Platform / architecture selection ─────────────────────────

    #[test]
    fn target_detection_reports_known_lanes() {
        let t = OfficeTarget::detect();
        assert_eq!(OfficeOs::current(), t.os);
        assert_eq!(OfficeArch::current(), t.arch);
        assert!(matches!(
            t.os,
            OfficeOs::Windows | OfficeOs::Linux | OfficeOs::Macos | OfficeOs::Other
        ));
        assert!(matches!(
            t.arch,
            OfficeArch::X86_64 | OfficeArch::Aarch64 | OfficeArch::Other
        ));
    }

    #[test]
    fn windows_x64_selects_the_proven_contract_plus_app_local_vc_runtime() {
        let plan = windows_plan();
        assert_eq!(plan.support, PlatformSupport::Published);
        assert_eq!(plan.artifact_format, ArchiveFormat::Zip);
        assert!(
            plan.artifact_name
                .starts_with("safeai-office-privacy-runtime-windows-x64")
        );
        assert_eq!(
            plan.required_runtime_files.len(),
            16,
            "12 proven pf-cli files + 4 app-local VC++ runtime DLLs"
        );
        assert!(plan.required_runtime_files.contains(&WINDOWS_RELATIVE_BINARY));
        for vc in WINDOWS_VC_RUNTIME_FILES {
            assert!(
                plan.required_runtime_files
                    .iter()
                    .any(|f| f.ends_with(vc)),
                "app-local VC++ runtime file {vc} must be part of the self-contained contract"
            );
        }
        for rel in plan.required_runtime_files {
            assert!(
                rel.starts_with(WINDOWS_RUNTIME_DIR),
                "{rel} must keep the proven archive layout"
            );
        }
        // The `api-ms-win-crt-*` / KERNEL32 OS forwarders are never required.
        for rel in plan.required_runtime_files {
            assert!(
                !rel.contains("api-ms-win-crt") && !rel.contains("KERNEL32"),
                "{rel} is an operating-system component, not part of the contract"
            );
        }
    }

    #[test]
    fn linux_x64_uses_the_bundled_bundle_contract() {
        let plan = linux_plan();
        assert_eq!(plan.support, PlatformSupport::AwaitingOfficeArtifact);
        assert_eq!(plan.artifact_format, ArchiveFormat::TarGz);
        assert_eq!(
            plan.required_runtime_files,
            &[
                "runtime/pf-cli",
                "runtime/ggml/src/libggml.so.0",
                "runtime/ggml/src/libggml-base.so.0",
                "runtime/bin/libggml-cpu-x64.so",
            ],
            "the four proven Linux files, at the paths the proof verifies"
        );
        assert_eq!(relative_binary_for(&plan), Some(LINUX_RELATIVE_BINARY));
        assert!(
            plan.required_runtime_files
                .iter()
                .all(|f| f.starts_with(LINUX_RUNTIME_DIR)),
            "every Linux runtime file must sit under the archive prefix"
        );
    }

    #[test]
    fn linux_x64_is_prepared_and_digest_pinned_but_not_published() {
        let _guard = test_env_guard();
        // SAFETY: serialised by the env lock.
        unsafe {
            std::env::remove_var(ENV_RUNTIME_URL_OVERRIDE);
        }
        let plan = linux_plan();

        // Prepared: a real, well-formed digest of the exact archive that will
        // be published. That the digest describes the prepared archive itself
        // is proven by the local Linux install run, not here.
        assert_eq!(plan.artifact_name, LINUX_ARTIFACT_NAME);
        assert_eq!(plan.artifact_sha256, Some(LINUX_ARTIFACT_SHA256));
        assert_eq!(plan.artifact_sha256.map(str::len), Some(64));
        assert!(
            plan.artifact_sha256
                .expect("Linux must be digest-pinned")
                .chars()
                .all(|c| c.is_ascii_hexdigit()),
            "the pinned Linux digest must be lowercase-agnostic hex"
        );
        assert_eq!(plan.provenance, CONTRACT_PROVENANCE);
        assert_eq!(plan.archive_prefix, LINUX_RUNTIME_DIR);
        assert!(
            plan.support_detail.contains("not published"),
            "the surface must be told why there is no install button: {}",
            plan.support_detail
        );

        // Not published: no URL, so the ordinary customer download still
        // refuses and the surface says it is waiting on a release asset rather
        // than offering a button that cannot work.
        assert!(plan.artifact_url.is_none());
        assert_eq!(resolved_url(&plan), None);
        assert_eq!(
            require_published_artifact(&plan),
            Err(InstallError::ArtifactNotPublished {
                artifact_name: LINUX_ARTIFACT_NAME
            })
        );
        let root = temp_root("linux-unpublished");
        assert_eq!(
            effective_state(Some(&root), &plan).0,
            OfficePrivacyState::AwaitingArtifact
        );

        // SAFETY: serialised by the env lock.
        unsafe {
            std::env::remove_var(ENV_RUNTIME_URL_OVERRIDE);
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn linux_x64_digest_gated_mirror_is_what_enables_the_local_proof() {
        let _guard = test_env_guard();
        // SAFETY: serialised by the env lock.
        unsafe {
            std::env::remove_var(ENV_RUNTIME_URL_OVERRIDE);
        }
        let plan = linux_plan();
        let root = temp_root("linux-mirror");

        assert_eq!(resolved_url(&plan), None);
        assert_eq!(
            effective_state(Some(&root), &plan).0,
            OfficePrivacyState::AwaitingArtifact
        );

        // A digest-pinned target may be mirrored, and that is exactly what the
        // local Linux proof relies on to serve the prepared archive.
        // SAFETY: serialised by the env lock.
        unsafe {
            std::env::set_var(ENV_RUNTIME_URL_OVERRIDE, "http://127.0.0.1:1/runtime.tar.gz");
        }
        assert_eq!(
            resolved_url(&plan).as_deref(),
            Some("http://127.0.0.1:1/runtime.tar.gz")
        );
        assert_eq!(require_published_artifact(&plan), Ok(()));
        // The digest is still pinned, so relocating the download cannot
        // substitute different content.
        assert_eq!(plan.artifact_sha256, Some(LINUX_ARTIFACT_SHA256));
        // And the surface now offers a real install path instead of "waiting".
        assert_eq!(
            effective_state(Some(&root), &plan).0,
            OfficePrivacyState::NotInstalled
        );

        // Withdrawing the override restores the refused, unpublished state.
        // SAFETY: serialised by the env lock.
        unsafe {
            std::env::remove_var(ENV_RUNTIME_URL_OVERRIDE);
        }
        assert_eq!(
            effective_state(Some(&root), &plan).0,
            OfficePrivacyState::AwaitingArtifact
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unproven_targets_are_unsupported() {
        for arch in [OfficeArch::X86_64, OfficeArch::Aarch64] {
            let plan = plan_for(OfficeTarget::new(OfficeOs::Macos, arch));
            assert_eq!(
                plan.support,
                PlatformSupport::Unsupported,
                "macOS {arch:?} has never been proven"
            );
            assert!(plan.required_runtime_files.is_empty());
            assert_eq!(relative_binary_for(&plan), None);
            assert_eq!(plan.support.as_str(), "unsupported");
        }
        let exotic = plan_for(OfficeTarget::new(OfficeOs::Windows, OfficeArch::Aarch64));
        assert_eq!(exotic.support, PlatformSupport::Unsupported);
        let other = plan_for(OfficeTarget::new(OfficeOs::Other, OfficeArch::Other));
        assert_eq!(other.support, PlatformSupport::Unsupported);
    }

    #[test]
    fn unsupported_platform_refuses_install_and_reports_unsupported_state() {
        let root = temp_root("unsupported");
        let plan = plan_for(OfficeTarget::new(OfficeOs::Macos, OfficeArch::Aarch64));
        let staging = stage_component(&root, &plan, "st", b"model");

        let err = finalize_install(&root, &staging, &plan, &"0".repeat(64), "t".into())
            .expect_err("unsupported platform must refuse");
        assert_eq!(err, InstallError::UnsupportedPlatform);
        assert_eq!(err.code(), "unsupported_platform");
        assert_eq!(
            require_published_artifact(&plan),
            Err(InstallError::UnsupportedPlatform)
        );

        assert!(!manifest_path(&root).exists(), "no manifest on an unsupported platform");
        assert!(!component_dir(&root).exists(), "nothing is promoted");
        assert_eq!(assess(Some(&root), &plan).0, OfficePrivacyState::Unsupported);
        let _ = std::fs::remove_dir_all(&root);
    }

    // ── Storage root ──────────────────────────────────────────────

    #[test]
    fn office_root_uses_office_userdata_not_desktop_storage() {
        let _guard = test_env_guard();
        // SAFETY: serialised by the env lock.
        unsafe {
            std::env::remove_var(ENV_ROOT_OVERRIDE);
            std::env::remove_var("XDG_CONFIG_HOME");
            if OfficeOs::current() != OfficeOs::Windows {
                std::env::set_var("HOME", "/home/tester");
            } else {
                std::env::set_var("APPDATA", "C:\\Users\\tester\\AppData\\Roaming");
            }
        }
        let root = office_userdata_root().expect("a userData root on this platform");
        let as_str = root.to_string_lossy().replace('\\', "/");

        assert!(
            as_str.ends_with("SafeAI Office"),
            "Office userData root must end with the Office app dir, got {as_str}"
        );
        assert!(
            !as_str.contains("privacy-models") && !as_str.contains("privacy-runtime"),
            "must never resolve into Desktop storage: {as_str}"
        );
        assert!(
            !as_str.to_lowercase().starts_with("/tmp"),
            "durable storage must not be a disposable temp path: {as_str}"
        );

        match OfficeOs::current() {
            OfficeOs::Linux => {
                assert_eq!(as_str, "/home/tester/.config/SafeAI Office");
                let privacy = office_privacy_root().unwrap();
                assert_eq!(
                    privacy.to_string_lossy().replace('\\', "/"),
                    "/home/tester/.config/SafeAI Office/office-privacy"
                );
            }
            OfficeOs::Macos => {
                assert_eq!(
                    as_str,
                    "/home/tester/Library/Application Support/SafeAI Office"
                );
            }
            OfficeOs::Windows => {
                assert_eq!(as_str, "C:/Users/tester/AppData/Roaming/SafeAI Office");
            }
            OfficeOs::Other => {}
        }
    }

    #[test]
    fn xdg_config_home_is_respected_on_linux() {
        let _guard = test_env_guard();
        if OfficeOs::current() != OfficeOs::Linux {
            return;
        }
        // SAFETY: serialised by the env lock.
        unsafe {
            std::env::remove_var(ENV_ROOT_OVERRIDE);
            std::env::set_var("XDG_CONFIG_HOME", "/custom/xdg");
        }
        assert_eq!(
            office_privacy_root().unwrap().to_string_lossy().replace('\\', "/"),
            "/custom/xdg/SafeAI Office/office-privacy"
        );
    }

    #[test]
    fn explicit_root_override_is_honoured_and_still_suffixed() {
        let _guard = test_env_guard();
        let dir = temp_root("override");
        // SAFETY: serialised by the env lock.
        unsafe {
            std::env::set_var(ENV_ROOT_OVERRIDE, &dir);
        }
        assert_eq!(office_privacy_root().unwrap(), dir.join(OFFICE_PRIVACY_DIR));
        // SAFETY: serialised by the env lock.
        unsafe {
            std::env::remove_var(ENV_ROOT_OVERRIDE);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Checksums ─────────────────────────────────────────────────

    #[test]
    fn sha256_matches_published_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(MODEL_SHA256.len(), 64, "pinned model digest is a SHA-256 hex string");
    }

    #[test]
    fn checksum_success_and_mismatch_rejection() {
        let dir = temp_root("checksum");
        let payload = dir.join("runtime.bin");
        std::fs::write(&payload, b"verified-payload").unwrap();
        let good = sha256_hex(b"verified-payload");

        assert!(verify_archive_digest(&payload, &good).is_ok());
        assert!(
            digests_match(&good.to_uppercase(), &good),
            "hex comparison is case-insensitive"
        );
        assert!(!digests_match("abc", "abcd"), "length mismatch never matches");

        let err = verify_archive_digest(&payload, &"0".repeat(64))
            .expect_err("a wrong digest must be rejected");
        match err {
            InstallError::ChecksumMismatch { expected, actual } => {
                assert_eq!(expected, "0".repeat(64));
                assert_eq!(actual, good);
                assert_eq!(
                    InstallError::ChecksumMismatch { expected, actual }.code(),
                    "checksum_mismatch"
                );
            }
            other => panic!("expected ChecksumMismatch, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_digest_matches_in_memory_digest() {
        let dir = temp_root("filedigest");
        let path = dir.join("blob");
        let bytes = vec![7u8; 300 * 1024];
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(sha256_file(&path).unwrap(), sha256_hex(&bytes));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_model_that_does_not_match_its_pinned_digest_is_rejected() {
        let root = temp_root("model-mismatch");
        let plan = windows_plan();
        let staging = stage_component(&root, &plan, "st", b"not-the-real-model");
        let err = finalize_install(&root, &staging, &plan, &"a".repeat(64), "t".into())
            .expect_err("a wrong model digest must be rejected");
        assert!(matches!(err, InstallError::ChecksumMismatch { .. }));
        assert!(!manifest_path(&root).exists(), "a rejected model leaves no manifest");
        assert!(!component_dir(&root).exists(), "a rejected model is not promoted");
        let _ = std::fs::remove_dir_all(&root);
    }

    // ── Required-file rejection ───────────────────────────────────

    #[test]
    fn missing_required_runtime_file_is_rejected() {
        let root = temp_root("missing-file");
        let plan = windows_plan();
        let staging = stage_component(&root, &plan, "st", b"model");

        let victim = staging.join(format!("{WINDOWS_RUNTIME_DIR}ggml-cpu-icelake.dll"));
        std::fs::remove_file(&victim).unwrap();

        let err = finalize_install(&root, &staging, &plan, &"a".repeat(64), "t".into())
            .expect_err("an incomplete runtime must be rejected");
        match err {
            InstallError::MissingRuntimeFiles(files) => {
                assert_eq!(files.len(), 1);
                assert!(files[0].ends_with("ggml-cpu-icelake.dll"));
                assert_eq!(
                    InstallError::MissingRuntimeFiles(files).code(),
                    "missing_runtime_files"
                );
            }
            other => panic!("expected MissingRuntimeFiles, got {other:?}"),
        }
        assert!(
            !manifest_path(&root).exists(),
            "a partial install must not produce a manifest"
        );
        assert!(
            !component_dir(&root).exists(),
            "a partial install must not be promoted into durable storage"
        );
        assert_eq!(
            assess(Some(&root), &plan).0,
            OfficePrivacyState::NotInstalled,
            "a leftover staging tree is never read as installed"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_vc_runtime_dll_is_rejected_so_a_clean_machine_cannot_fail_later() {
        let root = temp_root("missing-vc");
        let plan = windows_plan();
        let staging = stage_component(&root, &plan, "st", b"model");

        // The historical BUG-0009 shape: native deps absent on a clean machine.
        std::fs::remove_file(staging.join(format!("{WINDOWS_RUNTIME_DIR}VCRUNTIME140.dll"))).unwrap();

        let err = finalize_install(&root, &staging, &plan, &"a".repeat(64), "t".into())
            .expect_err("a runtime package without its native C++ deps must be rejected");
        match err {
            InstallError::MissingRuntimeFiles(files) => {
                assert_eq!(files.len(), 1);
                assert!(files[0].ends_with("VCRUNTIME140.dll"));
            }
            other => panic!("expected MissingRuntimeFiles, got {other:?}"),
        }
        assert!(!manifest_path(&root).exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_required_runtime_file_is_rejected() {
        let root = temp_root("empty-file");
        let plan = windows_plan();
        let staging = stage_component(&root, &plan, "st", b"model");
        std::fs::write(staging.join(format!("{WINDOWS_RUNTIME_DIR}ggml.dll")), b"").unwrap();

        let missing = missing_required_files(&staging, &plan);
        assert_eq!(missing.len(), 1, "a zero-byte runtime file counts as missing");
        assert!(missing[0].ends_with("ggml.dll"));
        let _ = std::fs::remove_dir_all(&root);
    }

    // ── Manifest contract ─────────────────────────────────────────

    #[test]
    fn manifest_paths_are_relative_and_traversal_free() {
        let plan = windows_plan();
        let manifest =
            OfficePrivacyManifest::build(&plan, &"a".repeat(64), "2026-09-15T00:00:00Z".into())
                .unwrap();
        assert!(manifest.paths_are_relative());

        let serialized = serde_json::to_string(&manifest).unwrap();
        assert!(
            !serialized.contains("http://") && !serialized.contains("https://"),
            "the manifest must not contain download URLs"
        );
        for value in [
            manifest.model.relative_path.as_str(),
            manifest.runtime.relative_binary.as_str(),
        ] {
            assert!(!value.starts_with('/'), "{value} must be relative");
            assert!(!value.contains(":\\"), "{value} must not be absolute");
        }

        let mut traversal = manifest.clone();
        traversal.model.relative_path = "../../etc/passwd".into();
        assert!(!traversal.paths_are_relative(), "traversal must be rejected");

        let mut absolute = manifest.clone();
        absolute.runtime.relative_binary = "/usr/bin/pf-cli".into();
        assert!(!absolute.paths_are_relative(), "absolute paths must be rejected");

        let mut windows_absolute = manifest;
        windows_absolute.runtime.relative_binary = "C:\\pwn\\pf-cli.exe".into();
        assert!(
            !windows_absolute.paths_are_relative(),
            "drive-qualified paths must be rejected"
        );
    }

    #[test]
    fn write_manifest_refuses_non_relative_paths() {
        let root = temp_root("bad-manifest");
        let plan = windows_plan();
        let mut manifest =
            OfficePrivacyManifest::build(&plan, &"a".repeat(64), "2026-09-15T00:00:00Z".into())
                .unwrap();
        manifest.model.relative_path = "../escape.gguf".into();
        assert!(write_manifest(&root, &manifest).is_err());
        assert!(!manifest_path(&root).exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn manifest_carries_every_required_field() {
        let plan = windows_plan();
        let manifest =
            OfficePrivacyManifest::build(&plan, &"b".repeat(64), "2026-09-15T12:34:56Z".into())
                .unwrap();
        let value = serde_json::to_value(&manifest).unwrap();

        for key in [
            "schema",
            "component_id",
            "component_version",
            "platform",
            "architecture",
            "installed_at",
        ] {
            assert!(value.get(key).is_some(), "manifest must carry {key}");
        }
        assert_eq!(value["component_id"], COMPONENT_ID);
        assert_eq!(value["schema"], MANIFEST_SCHEMA);
        assert_eq!(value["platform"], "windows");
        assert_eq!(value["architecture"], "x86_64");
        assert_eq!(value["model"]["relative_path"], relative_model_path());
        assert_eq!(value["model"]["sha256"], MODEL_SHA256);
        assert_eq!(value["runtime"]["relative_binary"], WINDOWS_RELATIVE_BINARY);
        assert_eq!(value["runtime"]["package_sha256"], "b".repeat(64));
        assert_eq!(
            value["runtime"]["required_files"].as_array().unwrap().len(),
            plan.required_runtime_files.len()
        );
        // There is deliberately no installing / failed / partial state field.
        assert!(value.get("state").is_none());
        assert!(value.get("download_url").is_none());
    }

    #[test]
    fn manifest_is_absent_and_state_is_not_installed_before_any_install() {
        let root = temp_root("no-manifest");
        assert!(read_manifest(&root).is_none());
        let (state, missing) = assess(Some(&root), &windows_plan());
        assert_eq!(state, OfficePrivacyState::NotInstalled);
        assert_eq!(state.as_str(), "not_installed");
        assert!(missing.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn corrupt_or_foreign_manifest_is_not_trusted() {
        let root = temp_root("corrupt-manifest");
        let plan = windows_plan();

        std::fs::write(manifest_path(&root), b"{ not json").unwrap();
        assert!(read_manifest(&root).is_none(), "unparseable manifest is ignored");
        assert_eq!(assess(Some(&root), &plan).0, OfficePrivacyState::NotInstalled);

        let mut wrong_schema =
            OfficePrivacyManifest::build(&plan, &"c".repeat(64), "t".into()).unwrap();
        wrong_schema.schema = MANIFEST_SCHEMA + 1;
        std::fs::write(manifest_path(&root), serde_json::to_string(&wrong_schema).unwrap()).unwrap();
        assert!(read_manifest(&root).is_none(), "a foreign schema is ignored");

        let mut wrong_component =
            OfficePrivacyManifest::build(&plan, &"c".repeat(64), "t".into()).unwrap();
        wrong_component.component_id = "some-other-component".into();
        std::fs::write(
            manifest_path(&root),
            serde_json::to_string(&wrong_component).unwrap(),
        )
        .unwrap();
        assert!(read_manifest(&root).is_none(), "another component's manifest is ignored");
        let _ = std::fs::remove_dir_all(&root);
    }

    // ── Completed install ─────────────────────────────────────────

    #[test]
    fn completed_install_produces_manifest_and_installed_state() {
        let root = temp_root("completed");
        let plan = windows_plan();
        let model = b"stand-in-model-payload";
        let model_digest = sha256_hex(model);
        let staging = stage_component(&root, &plan, "st", model);
        let archive_digest = "d".repeat(64);

        let manifest = finalize_install_with_model_digest(
            &root,
            &staging,
            &plan,
            &archive_digest,
            &model_digest,
            "2026-09-15T10:00:00Z".into(),
        )
        .expect("a complete, verified component installs");

        assert!(manifest_path(&root).exists(), "completed install writes the manifest");
        assert!(component_dir(&root).exists(), "completed install promotes the component");
        assert!(!staging.exists(), "staging is consumed by the promote");

        let reread = read_manifest(&root).expect("manifest must round-trip");
        assert_eq!(reread, manifest);
        assert!(reread.paths_are_relative());
        assert_eq!(reread.runtime.package_sha256, archive_digest);
        assert_eq!(reread.runtime.required_files.len(), plan.required_runtime_files.len());
        assert_eq!(reread.installed_at, "2026-09-15T10:00:00Z");

        // Every recorded file must exist under the durable component directory.
        for rel in &reread.runtime.required_files {
            assert!(component_dir(&root).join(rel).is_file(), "{rel} must exist");
        }

        let (state, missing) = assess(Some(&root), &plan);
        assert_eq!(state, OfficePrivacyState::Installed);
        assert_eq!(state.as_str(), "installed");
        assert!(missing.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_manifest_whose_files_vanished_does_not_read_as_installed() {
        let root = temp_root("vanished");
        let plan = linux_plan();
        let model = b"payload";
        let staging = stage_component(&root, &plan, "st", model);
        finalize_install_with_model_digest(
            &root,
            &staging,
            &plan,
            &"e".repeat(64),
            &sha256_hex(model),
            now_rfc3339(),
        )
        .unwrap();
        assert_eq!(assess(Some(&root), &plan).0, OfficePrivacyState::Installed);

        // Simulate an out-of-band deletion of the runtime binary.
        std::fs::remove_file(component_dir(&root).join(LINUX_RELATIVE_BINARY)).unwrap();
        let (state, missing) = assess(Some(&root), &plan);
        assert_eq!(state, OfficePrivacyState::NotInstalled);
        assert_eq!(missing, vec![LINUX_RELATIVE_BINARY.to_string()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn partial_staging_never_produces_a_manifest() {
        let root = temp_root("partial");
        let plan = linux_plan();
        let staging = staging_dir(&root, "partial");
        std::fs::create_dir_all(staging.join("runtime")).unwrap();
        // Only pf-cli is present; the ggml libs are missing.
        std::fs::write(staging.join(LINUX_RELATIVE_BINARY), b"stub").unwrap();

        let err = finalize_install_with_model_digest(
            &root,
            &staging,
            &plan,
            &"f".repeat(64),
            &sha256_hex(b"x"),
            "t".into(),
        )
        .expect_err("a partial staging tree must be rejected");
        assert!(matches!(err, InstallError::MissingRuntimeFiles(_)));
        assert!(!manifest_path(&root).exists(), "no manifest from a partial install");
        assert!(!component_dir(&root).exists(), "no component from a partial install");
        assert!(!root.join("manifest.json").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reinstall_replaces_the_previous_component_without_leftovers() {
        let root = temp_root("reinstall");
        let plan = linux_plan();
        let binary = component_dir(&root).join(LINUX_RELATIVE_BINARY);

        let first = stage_component(&root, &plan, "first", b"v1");
        std::fs::write(first.join(LINUX_RELATIVE_BINARY), b"v1").unwrap();
        finalize_install_with_model_digest(
            &root,
            &first,
            &plan,
            &"1".repeat(64),
            &sha256_hex(b"v1"),
            "t1".into(),
        )
        .unwrap();
        assert_eq!(std::fs::read(&binary).unwrap(), b"v1");

        let second = stage_component(&root, &plan, "second", b"v2");
        std::fs::write(second.join(LINUX_RELATIVE_BINARY), b"v2").unwrap();
        finalize_install_with_model_digest(
            &root,
            &second,
            &plan,
            &"2".repeat(64),
            &sha256_hex(b"v2"),
            "t2".into(),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(&binary).unwrap(),
            b"v2",
            "the promoted component must be the new one"
        );
        assert_eq!(read_manifest(&root).unwrap().runtime.package_sha256, "2".repeat(64));

        let leftovers: Vec<String> = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(".previous-"))
            .collect();
        assert!(leftovers.is_empty(), "no backup left behind: {leftovers:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    // ── Published-artifact gate ───────────────────────────────────

    #[test]
    #[test]
    fn windows_is_published_with_the_real_office_owned_artifact() {
        // Guards the exact published values. If this fails, the Windows
        // installer is pointing somewhere other than the reviewed asset.
        let _guard = test_env_guard();
        // SAFETY: serialised by the env lock.
        unsafe {
            std::env::remove_var(ENV_RUNTIME_URL_OVERRIDE);
        }
        let plan = windows_plan();
        assert_eq!(plan.support, PlatformSupport::Published);
        assert_eq!(plan.support.as_str(), "published");
        assert_eq!(plan.artifact_name, WINDOWS_ARTIFACT_NAME);
        assert_eq!(
            plan.artifact_sha256,
            Some("0bc2e0aa0177f2b82b3ae385f3adc1c65f515bac1730549840e118a34b08886b")
        );
        assert_eq!(plan.artifact_sha256.map(str::len), Some(64));
        assert!(plan.artifact_sha256.unwrap().chars().all(|c| c.is_ascii_hexdigit()));

        let url = plan.artifact_url.expect("Windows must have a published URL");
        assert_eq!(url, WINDOWS_ARTIFACT_URL);
        // The Office-owned repository, never the SafeAI Desktop runtime repo.
        assert!(
            url.starts_with("https://github.com/fabiofurlano/safeai-office-runtime/releases/download/")
        );
        assert!(!url.contains("safeai-runtime"), "must not depend on the Desktop runtime repo");
        assert!(url.contains(OFFICE_PRIVACY_RELEASE_TAG), "{url}");
        assert!(url.ends_with(WINDOWS_ARTIFACT_NAME), "{url}");
        // A usable URL: no whitespace from any line continuation.
        assert!(!url.chars().any(char::is_whitespace), "{url}");
        assert_eq!(
            url,
            "https://github.com/fabiofurlano/safeai-office-runtime/releases/download/office-privacy-v1.0.0/safeai-office-privacy-runtime-windows-x64-v1.0.0.zip"
        );

        // The install gate now passes for Windows without any override.
        assert_eq!(resolved_url(&plan).as_deref(), Some(WINDOWS_ARTIFACT_URL));
        assert_eq!(require_published_artifact(&plan), Ok(()));

        // A mirror override can relocate the download, but only because the
        // digest is pinned — and the digest is still what decides acceptance.
        // SAFETY: serialised by the env lock.
        unsafe {
            std::env::set_var(ENV_RUNTIME_URL_OVERRIDE, "https://mirror.invalid/same.zip");
        }
        assert_eq!(
            resolved_url(&plan).as_deref(),
            Some("https://mirror.invalid/same.zip"),
            "a pinned-digest target may be mirrored"
        );
        assert_eq!(
            plan.artifact_sha256,
            Some(WINDOWS_ARTIFACT_SHA256),
            "mirroring must not unpin the digest"
        );
        // SAFETY: serialised by the env lock.
        unsafe {
            std::env::remove_var(ENV_RUNTIME_URL_OVERRIDE);
        }
        assert_eq!(resolved_url(&plan).as_deref(), Some(WINDOWS_ARTIFACT_URL));
    }

    #[test]
    fn unpublished_targets_still_refuse_the_customer_download() {
        let _guard = test_env_guard();
        // SAFETY: serialised by the env lock.
        unsafe {
            std::env::remove_var(ENV_RUNTIME_URL_OVERRIDE);
        }
        // Linux is proven and digest-pinned but its asset is not published, so
        // there is still no URL to download from; macOS has never been proven
        // and has no digest at all.
        for target in [
            OfficeTarget::new(OfficeOs::Linux, OfficeArch::X86_64),
            OfficeTarget::new(OfficeOs::Macos, OfficeArch::X86_64),
            OfficeTarget::new(OfficeOs::Macos, OfficeArch::Aarch64),
        ] {
            let plan = plan_for(target);
            assert_ne!(
                plan.support,
                PlatformSupport::Published,
                "{target:?} must not claim to be published"
            );
            assert!(plan.artifact_url.is_none(), "{target:?}");
            assert_eq!(resolved_url(&plan), None, "{target:?}");
            assert!(
                require_published_artifact(&plan).is_err(),
                "{target:?} must refuse the download"
            );
        }

        // Linux specifically: proven contract and a real pinned digest, but
        // still a withheld download, and the refusal names the asset that still
        // needs publishing.
        let linux = linux_plan();
        assert_eq!(linux.support, PlatformSupport::AwaitingOfficeArtifact);
        assert_eq!(linux.artifact_sha256, Some(LINUX_ARTIFACT_SHA256));
        assert!(linux.artifact_url.is_none());
        let err = require_published_artifact(&linux)
            .expect_err("an unpublished target must refuse the download");
        assert_eq!(err.code(), "artifact_not_published");
        assert!(err.user_message().contains(linux.artifact_name));
    }

    #[test]
    fn runtime_url_override_is_ignored_until_a_digest_is_pinned() {
        let _guard = test_env_guard();
        // Every lane that has no proven runtime carries no pinned digest, so an
        // override must never be enough to start an unverified install there.
        for target in [
            OfficeTarget::new(OfficeOs::Macos, OfficeArch::X86_64),
            OfficeTarget::new(OfficeOs::Macos, OfficeArch::Aarch64),
            OfficeTarget::new(OfficeOs::Windows, OfficeArch::Aarch64),
            OfficeTarget::new(OfficeOs::Other, OfficeArch::Other),
        ] {
            let plan = plan_for(target);
            assert!(plan.artifact_sha256.is_none(), "{target:?}");
            // SAFETY: serialised by the env lock.
            unsafe {
                std::env::set_var(ENV_RUNTIME_URL_OVERRIDE, "file:///tmp/fake-runtime.zip");
            }
            assert_eq!(
                resolved_url(&plan),
                None,
                "{target:?}: an override without a pinned digest must not enable an \
                 unverified install"
            );
            assert_eq!(
                require_published_artifact(&plan),
                Err(InstallError::UnsupportedPlatform),
                "{target:?}: an unproven platform stays refused even with an override"
            );
        }
        // SAFETY: serialised by the env lock.
        unsafe {
            std::env::remove_var(ENV_RUNTIME_URL_OVERRIDE);
        }
    }

    // ── Shell-argument safety ─────────────────────────────────────

    /// Undo [`powershell_literal`] the way PowerShell would, so a test can
    /// prove the literal decodes back to exactly the original path.
    fn powershell_unescape(literal: &str) -> String {
        assert!(literal.len() >= 2, "a literal is always quoted: {literal}");
        let body = &literal[1..literal.len() - 1];
        body.replace("''", "'")
    }

    /// True only when every quote inside the body is part of a doubled pair,
    /// i.e. no quote can terminate the PowerShell literal.
    fn has_no_unescaped_quote(literal: &str) -> bool {
        let body = &literal[1..literal.len() - 1];
        !body.replace("''", "").contains('\'')
    }

    #[test]
    fn powershell_literals_cannot_break_out_of_quoting() {
        assert_eq!(
            powershell_literal(Path::new("C:\\plain\\dir")),
            "'C:\\plain\\dir'"
        );
        assert!(has_no_unescaped_quote(&powershell_literal(Path::new(
            "C:\\plain\\dir"
        ))));

        // A name containing an apostrophe must be doubled, never allowed to
        // terminate the literal. The durable root is reachable from an
        // environment override, so this input is not purely hypothetical.
        let quoted = powershell_literal(Path::new("C:\\Users\\o'brien\\x"));
        assert_eq!(quoted, "'C:\\Users\\o''brien\\x'");
        assert!(has_no_unescaped_quote(&quoted));
        assert_eq!(
            powershell_unescape(&quoted),
            "C:\\Users\\o'brien\\x",
            "the literal must decode back to the exact original path"
        );

        // A path crafted to close the literal and start a new statement stays
        // inert: the payload never lands outside the quotes.
        let payload = "/tmp/a'; Remove-Item -Recurse C:\\ ;'";
        let hostile = powershell_literal(Path::new(payload));
        assert!(hostile.starts_with('\'') && hostile.ends_with('\''));
        assert!(
            has_no_unescaped_quote(&hostile),
            "an unescaped quote must never terminate the PowerShell literal: {hostile}"
        );
        assert_eq!(
            powershell_unescape(&hostile),
            payload,
            "the injected payload must stay inside the literal, verbatim"
        );
    }

    // ── Contract shape ────────────────────────────────────────────

    #[test]
    fn every_required_file_sits_under_the_archive_prefix() {
        for plan in [windows_plan(), linux_plan()] {
            assert!(!plan.required_runtime_files.is_empty());
            for rel in plan.required_runtime_files {
                assert!(
                    rel.starts_with(plan.archive_prefix),
                    "{rel} must live under {}",
                    plan.archive_prefix
                );
            }
        }
    }

    #[test]
    fn accessors_are_stable_strings() {
        assert_eq!(OfficeOs::Windows.as_str(), "windows");
        assert_eq!(OfficeOs::Macos.as_str(), "macos");
        assert_eq!(OfficeArch::Aarch64.as_str(), "aarch64");
        assert_eq!(ArchiveFormat::Zip.as_str(), "zip");
        assert_eq!(ArchiveFormat::TarGz.as_str(), "tar.gz");
        assert_eq!(PlatformSupport::Published.as_str(), "published");
        assert_eq!(
            PlatformSupport::AwaitingOfficeArtifact.as_str(),
            "awaiting_office_artifact"
        );
        assert_eq!(OfficePrivacyState::AwaitingArtifact.as_str(), "awaiting_artifact");
        assert_eq!(COMPONENT_ID, "safeai-office-privacy-filter");
        assert_eq!(MANIFEST_FILE, "manifest.json");
        assert_eq!(COMPONENT_DIR, "component");
        assert_eq!(OFFICE_PRIVACY_DIR, "office-privacy");
        assert_eq!(OFFICE_APP_DIR, "SafeAI Office");
    }

    #[test]
    fn unsupported_plans_cannot_build_a_manifest() {
        let plan = plan_for(OfficeTarget::new(OfficeOs::Macos, OfficeArch::Aarch64));
        let err = OfficePrivacyManifest::build(&plan, "x", "t".into())
            .expect_err("an unsupported target has no binary to record");
        assert_eq!(err, InstallError::UnsupportedPlatform);
    }

    #[test]
    fn error_messages_are_user_facing_and_path_free() {
        let cases = [
            InstallError::UnsupportedPlatform,
            InstallError::ArtifactNotPublished {
                artifact_name: "asset.zip",
            },
            InstallError::Download("connection reset".into()),
            InstallError::ChecksumMismatch {
                expected: "a".into(),
                actual: "b".into(),
            },
            InstallError::Extraction("bad archive".into()),
            InstallError::MissingRuntimeFiles(vec!["runtime/a.dll".into()]),
            InstallError::Io("permission denied".into()),
            InstallError::Promote("busy".into()),
        ];
        for err in cases {
            let message = err.user_message();
            assert!(!message.is_empty(), "{err:?} must have a message");
            assert!(
                !message.contains("/home/") && !message.contains("C:\\"),
                "{message} must not leak an absolute machine path"
            );
            assert!(!err.code().is_empty());
        }
    }

    // ── Timestamp ─────────────────────────────────────────────────

    #[test]
    fn rfc3339_timestamp_is_utc_and_correct() {
        let epoch = std::time::UNIX_EPOCH;
        assert_eq!(format_rfc3339_utc(epoch), "1970-01-01T00:00:00Z");
        let known = epoch + std::time::Duration::from_secs(1_770_000_000);
        assert_eq!(format_rfc3339_utc(known), "2026-02-02T02:40:00Z");
        let formatted = now_rfc3339();
        assert_eq!(formatted.len(), 20, "{formatted}");
        assert!(formatted.ends_with('Z'), "{formatted}");
        assert_eq!(&formatted[4..5], "-");
    }
}
