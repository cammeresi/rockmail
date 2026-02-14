use std::process::Command;

fn main() {
    let ver = env!("CARGO_PKG_VERSION");
    let hash = Command::new("git")
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let full = if hash.is_empty() {
        ver.to_string()
    } else {
        format!("{ver} ({hash})")
    };
    println!("cargo:rustc-env=VERSION={full}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
}
