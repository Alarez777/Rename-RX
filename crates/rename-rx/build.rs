use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const PACKAGES: &[(&str, &str)] = &[
    ("egui", "RENAME_RX_EGUI_VERSION"),
    ("eframe", "RENAME_RX_EFRAME_VERSION"),
    ("egui_extras", "RENAME_RX_EGUI_EXTRAS_VERSION"),
    ("winit", "RENAME_RX_WINIT_VERSION"),
    ("glow", "RENAME_RX_GLOW_VERSION"),
    ("regex", "RENAME_RX_REGEX_VERSION"),
];

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let lock_path = manifest_dir.join("../../Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock_path.display());

    let lock = fs::read_to_string(&lock_path).unwrap_or_default();

    for (package, variable) in PACKAGES {
        let version = package_version(&lock, package).unwrap_or_else(|| "unknown".to_owned());
        println!("cargo:rustc-env={variable}={version}");
    }

    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let rustc_version = Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            text.split_whitespace().nth(1).map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=RENAME_RX_RUSTC_VERSION={rustc_version}");
}

fn package_version(lock: &str, package: &str) -> Option<String> {
    let mut name: Option<&str> = None;

    for line in lock.lines() {
        let line = line.trim();

        if line == "[[package]]" {
            name = None;
        } else if let Some(value) = line.strip_prefix("name = \"") {
            name = value.strip_suffix('"');
        } else if let Some(value) = line.strip_prefix("version = \"")
            && name == Some(package)
        {
            return value.strip_suffix('"').map(str::to_owned);
        }
    }

    None
}
