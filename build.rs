//! Records the commit the program is built from, and gives the Windows exe its icon.

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/mo4.ico");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=src");
    if let Some(commit) = commit() {
        println!("cargo:rustc-env=MO4_COMMIT={commit}");
    }
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/mo4.ico");
        if let Err(e) = resource.compile() {
            println!("cargo:warning=the exe has no icon: {e}");
        }
    }
}

/// The short commit, with "-dirty" when tracked files differ from it. `None` outside
/// the repository (a source archive) or without git, and then no commit is shown.
fn commit() -> Option<String> {
    // Only this folder's own repository counts, not one it happens to be unpacked in.
    if !Path::new(".git").exists() {
        return None;
    }
    // Both move when a commit is made or another one is checked out.
    for moved in [".git/HEAD", ".git/logs/HEAD"] {
        if Path::new(moved).exists() {
            println!("cargo:rerun-if-changed={moved}");
        }
    }
    let git = |args: &[&str]| {
        let out = Command::new("git").args(args).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let hash = git(&["rev-parse", "--short=7", "HEAD"]).filter(|h| !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit()))?;
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    Some(if dirty { format!("{hash}-dirty") } else { hash })
}
