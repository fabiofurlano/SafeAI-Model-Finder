use std::io::Write;
use std::path::Path;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let ui_dir = Path::new(&manifest_dir).join("src/ui");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("web_assets.rs");

    let mut entries: Vec<(String, String, String)> = Vec::new();

    if ui_dir.is_dir() {
        for entry in walkdir::WalkDir::new(&ui_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            // ponytail: forward slashes are valid Rust string content and work
            // in include_bytes! on Windows and Linux; backslashes would become
            // invalid escape sequences in the generated literal.
            let abs_path = entry.path().to_string_lossy().replace('\\', "/");
            let rel = entry.path().strip_prefix(&ui_dir).unwrap();
            // ponytail: also normalize the public URL path; on Windows
            // rel.display() would emit backslashes, which are invalid escape
            // sequences in the generated string literal and wrong as a URL.
            let path_str = format!("/ui/{}", rel.display()).replace('\\', "/");
            let content_type = mime_guess_from_path(&path_str);
            entries.push((path_str, content_type, abs_path));
        }
    }

    let mut f = std::fs::File::create(&dest_path).unwrap();

    writeln!(f, "#[derive(Clone, Copy)]").unwrap();
    writeln!(f, "pub struct EmbeddedAsset {{").unwrap();
    writeln!(f, "    pub path: &'static str,").unwrap();
    writeln!(f, "    pub content_type: &'static str,").unwrap();
    writeln!(f, "    pub bytes: &'static [u8],").unwrap();
    writeln!(f, "}}").unwrap();

    writeln!(f, "pub const EMBEDDED_WEB_ASSETS: &[EmbeddedAsset] = &[").unwrap();
    for (path, content_type, abs_path) in &entries {
        writeln!(f, "    EmbeddedAsset {{").unwrap();
        writeln!(f, "        path: \"{path}\",").unwrap();
        writeln!(f, "        content_type: \"{content_type}\",").unwrap();
        writeln!(f, "        bytes: include_bytes!(\"{abs_path}\"),").unwrap();
        writeln!(f, "    }},").unwrap();
    }
    writeln!(f, "];").unwrap();
}

fn mime_guess_from_path(path: &str) -> String {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
    .to_string()
}
