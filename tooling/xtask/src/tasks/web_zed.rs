#![allow(clippy::disallowed_methods, reason = "tooling is exempt")]

use std::fs::File;
use std::io::Write;
use std::process::Command;

use anyhow::{Context as _, Result, bail};
use clap::Parser;

#[derive(Parser)]
pub struct WebZedArgs {
    #[arg(long)]
    pub release: bool,
    #[arg(long, default_value = "8080")]
    pub port: u16,
    #[arg(long)]
    pub no_serve: bool,
}

fn check_program(binary: &str, install_hint: &str) -> Result<()> {
    match Command::new(binary).arg("--version").output() {
        Ok(output) if output.status.success() => Ok(()),
        _ => bail!("`{binary}` not found. Install with: {install_hint}"),
    }
}

pub fn run_web_zed(args: WebZedArgs) -> Result<()> {
    check_program("wasm-bindgen", "cargo install wasm-bindgen-cli")?;

    let profile = if args.release { "release" } else { "debug" };
    let output_directory = "target/web-zed";
    let wasm_path = format!("target/wasm32-unknown-unknown/{profile}/zed.wasm");

    let mut build_command = Command::new("cargo");
    build_command
        .env("RUSTUP_TOOLCHAIN", "nightly")
        .args(["build", "-p", "zed", "--target", "wasm32-unknown-unknown"]);
    if args.release {
        build_command.arg("--release");
    }

    let status = build_command
        .status()
        .context("failed to run cargo build for zed wasm")?;
    if !status.success() {
        bail!("cargo build failed with status {status}");
    }

    std::fs::create_dir_all(output_directory)
        .with_context(|| format!("failed to create {output_directory}"))?;

    let bindgen_status = Command::new("wasm-bindgen")
        .args([
            &wasm_path,
            "--target",
            "web",
            "--no-typescript",
            "--out-dir",
            output_directory,
            "--out-name",
            "zed",
        ])
        .status()
        .context("failed to run wasm-bindgen")?;
    if !bindgen_status.success() {
        bail!("wasm-bindgen failed with status {bindgen_status}");
    }

    let index_html_path = format!("{output_directory}/index.html");
    let mut index_html_file =
        File::create(&index_html_path).with_context(|| format!("failed to create {index_html_path}"))?;
    index_html_file
        .write_all(
            br#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Zed (Web)</title>
  <style>
    html, body { width: 100%; height: 100%; margin: 0; }
    body { background: #11111b; color: #cdd6f4; }
  </style>
</head>
<body>
  <script type="module">
    import init from './zed.js';
    await init();
  </script>
</body>
</html>
"#,
        )
        .with_context(|| format!("failed to write {index_html_path}"))?;

    if args.no_serve {
        eprintln!("Built zed web app in {output_directory}/");
        return Ok(());
    }

    eprintln!("Serving zed web app on http://127.0.0.1:{}", args.port);

    let server_script = format!(
        r#"
import http.server
class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory="{output_directory}", **kwargs)
    def end_headers(self):
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        super().end_headers()
http.server.HTTPServer(("127.0.0.1", {port}), Handler).serve_forever()
"#,
        port = args.port,
    );

    let server_status = Command::new("python3")
        .args(["-c", &server_script])
        .status()
        .context("failed to run python3 server")?;

    if !server_status.success() {
        bail!("python3 server exited with status {server_status}");
    }

    Ok(())
}
