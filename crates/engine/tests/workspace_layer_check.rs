//! Workspace Architecture Layer Enforcement Test
//!
//! Executed automatically by `cargo test --workspace`.
//! Verifies that:
//! 1. Lower crates never import upward or across disallowed peer borders:
//!    - Layer 1 (`core`) depends on nothing in workspace.
//!    - Layer 2 (`audio`, `platform`) depends only on Layer 1 (`core`).
//!    - Layer 3 (`media`, `render`) depends only on lower layers (`core`, `platform`).
//!    - Layer 4 (`engine`) depends only on lower layers (`core`, `audio`, `platform`, `media`, `render`).
//!    - Layer 5 (`app`, `cli`) depends only on lower layers.
//! 2. Nothing below `ventacore-app` depends on the interface toolkit (`egui`, `eframe`, `winit`).
//! 3. Nothing outside `ventacore-platform` names a codec library (`ffmpeg-next`, `ffmpeg-sys-next`, `rsmpeg`).
//! 4. Deliberately wrong sample manifests are proven to fail validation.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CrateRule {
    pub crate_dir: &'static str,
    pub crate_name: &'static str,
    pub allowed_workspace_deps: &'static [&'static str],
    pub allow_ui_toolkit: bool,
    pub allow_codec_bindings: bool,
}

pub const LAYER_RULES: &[CrateRule] = &[
    CrateRule {
        crate_dir: "crates/core",
        crate_name: "ventacore-core",
        allowed_workspace_deps: &[],
        allow_ui_toolkit: false,
        allow_codec_bindings: false,
    },
    CrateRule {
        crate_dir: "crates/audio",
        crate_name: "ventacore-audio",
        allowed_workspace_deps: &["ventacore-core"],
        allow_ui_toolkit: false,
        allow_codec_bindings: false,
    },
    CrateRule {
        crate_dir: "crates/platform",
        crate_name: "ventacore-platform",
        allowed_workspace_deps: &["ventacore-core"],
        allow_ui_toolkit: false,
        allow_codec_bindings: true,
    },
    CrateRule {
        crate_dir: "crates/media",
        crate_name: "ventacore-media",
        allowed_workspace_deps: &["ventacore-core", "ventacore-platform"],
        allow_ui_toolkit: false,
        allow_codec_bindings: false,
    },
    CrateRule {
        crate_dir: "crates/render",
        crate_name: "ventacore-render",
        allowed_workspace_deps: &["ventacore-core", "ventacore-platform"],
        allow_ui_toolkit: false,
        allow_codec_bindings: false,
    },
    CrateRule {
        crate_dir: "crates/engine",
        crate_name: "ventacore-engine",
        allowed_workspace_deps: &[
            "ventacore-core",
            "ventacore-audio",
            "ventacore-platform",
            "ventacore-media",
            "ventacore-render",
        ],
        allow_ui_toolkit: false,
        allow_codec_bindings: false,
    },
    CrateRule {
        crate_dir: "crates/app",
        crate_name: "ventacore-app",
        allowed_workspace_deps: &[
            "ventacore-core",
            "ventacore-audio",
            "ventacore-platform",
            "ventacore-media",
            "ventacore-render",
            "ventacore-engine",
        ],
        allow_ui_toolkit: true,
        allow_codec_bindings: false,
    },
    CrateRule {
        crate_dir: "crates/cli",
        crate_name: "ventacore-cli",
        allowed_workspace_deps: &[
            "ventacore-core",
            "ventacore-audio",
            "ventacore-platform",
            "ventacore-media",
            "ventacore-render",
            "ventacore-engine",
        ],
        allow_ui_toolkit: false,
        allow_codec_bindings: false,
    },
];

pub const ALL_WORKSPACE_CRATES: &[&str] = &[
    "ventacore-core",
    "ventacore-audio",
    "ventacore-platform",
    "ventacore-media",
    "ventacore-render",
    "ventacore-engine",
    "ventacore-app",
    "ventacore-cli",
];

/// Validates a Cargo.toml manifest against architectural layer rules.
pub fn validate_manifest(rule: &CrateRule, manifest_content: &str) -> Result<(), String> {
    // Check forbidden workspace dependencies
    for candidate in ALL_WORKSPACE_CRATES {
        if *candidate != rule.crate_name {
            let dep_pattern = format!("{candidate} ");
            let dep_pattern_eq = format!("{candidate}=");
            let dep_pattern_quote = format!("\"{candidate}\"");
            if (manifest_content.contains(&dep_pattern)
                || manifest_content.contains(&dep_pattern_eq)
                || manifest_content.contains(&dep_pattern_quote))
                && !rule.allowed_workspace_deps.contains(candidate)
            {
                return Err(format!(
                    "Layer violation in {}: forbidden upward or peer dependency on {}",
                    rule.crate_dir, candidate
                ));
            }
        }
    }

    // Check UI toolkit boundary
    if !rule.allow_ui_toolkit {
        for ui_crate in &["egui", "eframe", "winit"] {
            if manifest_content.contains(ui_crate) {
                return Err(format!(
                    "Layer violation in {}: lower module must never depend on UI toolkit '{}'",
                    rule.crate_dir, ui_crate
                ));
            }
        }
    }

    // Check Codec boundary
    if !rule.allow_codec_bindings {
        for codec_crate in &["ffmpeg-next", "ffmpeg-sys-next", "rsmpeg"] {
            if manifest_content.contains(codec_crate) {
                return Err(format!(
                    "Layer violation in {}: only ventacore-platform may reference codec library '{}'",
                    rule.crate_dir, codec_crate
                ));
            }
        }
    }

    Ok(())
}

#[test]
fn test_enforce_workspace_dependency_direction_and_borders() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("Failed to locate workspace root from crates/engine");

    for rule in LAYER_RULES {
        let cargo_toml_path = workspace_root.join(rule.crate_dir).join("Cargo.toml");
        let manifest = fs::read_to_string(&cargo_toml_path)
            .unwrap_or_else(|_| panic!("Missing Cargo.toml at {:?}", cargo_toml_path));

        if let Err(e) = validate_manifest(rule, &manifest) {
            panic!("Real workspace failed architectural check: {}", e);
        }
    }
}

#[test]
fn test_proves_layer_enforcement_fails_on_deliberately_wrong_samples() {
    // 1. Core illegally depending on Engine (upward dependency)
    let bad_core_manifest = r#"
        [package]
        name = "ventacore-core"
        version = "0.1.0"

        [dependencies]
        ventacore-engine = { path = "../engine" }
    "#;
    let core_rule = &LAYER_RULES[0];
    let result = validate_manifest(core_rule, bad_core_manifest);
    assert!(
        result.is_err(),
        "Must fail when core depends on engine upward"
    );
    assert!(result.unwrap_err().contains("forbidden upward"));

    // 2. Audio illegally depending on UI toolkit
    let bad_audio_manifest = r#"
        [package]
        name = "ventacore-audio"
        version = "0.1.0"

        [dependencies]
        ventacore-core = { path = "../core" }
        egui = "0.28"
    "#;
    let audio_rule = &LAYER_RULES[1];
    let result = validate_manifest(audio_rule, bad_audio_manifest);
    assert!(result.is_err(), "Must fail when audio depends on egui");
    assert!(result.unwrap_err().contains("UI toolkit"));

    // 3. Media illegally referencing ffmpeg directly (violating Charter rule 4)
    let bad_media_manifest = r#"
        [package]
        name = "ventacore-media"
        version = "0.1.0"

        [dependencies]
        ventacore-core = { path = "../core" }
        ventacore-platform = { path = "../platform" }
        ffmpeg-next = "7.0"
    "#;
    let media_rule = &LAYER_RULES[3];
    let result = validate_manifest(media_rule, bad_media_manifest);
    assert!(
        result.is_err(),
        "Must fail when media references ffmpeg directly"
    );
    assert!(result.unwrap_err().contains("codec library"));
}
