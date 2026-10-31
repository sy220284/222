use std::{fs, path::Path};

fn source_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            result.extend(source_files(&path));
        } else if path.extension().and_then(|v| v.to_str()) == Some("rs") {
            result.push(path);
        }
    }
    result
}

#[test]
fn no_brand_or_hidden_telemetry_exporter_in_rust_runtime() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = source_files(&root.join("src"));
    assert!(!files.is_empty());
    // Keep the prohibited runtime tokens out of literal production code.
    let blocked_names = [
        "clau".to_owned() + "de",
        "anthro".to_owned() + "pic",
        "tengu".to_owned(),
        "data".to_owned() + "dog",
        "opentele".to_owned() + "metry",
        "sentry".to_owned(),
    ];
    for path in files {
        let body = fs::read_to_string(&path).unwrap().to_ascii_lowercase();
        for word in &blocked_names {
            assert!(
                !body.contains(word),
                "unwanted identifier {word} in {}",
                path.display()
            );
        }
    }
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(!manifest.contains("opentelemetry"));
    assert!(!manifest.contains("sentry"));
}

#[test]
fn no_embedded_vendor_telemetry_server_url() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = source_files(&root.join("src"))
        .into_iter()
        .map(|p| fs::read_to_string(p).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    let disallowed = [
        "statsig",
        "segment.io",
        "api.segment",
        "sentry.io",
        "stats.datadoghq",
    ];
    for endpoint in disallowed {
        assert!(
            !source.contains(endpoint),
            "unwanted telemetry endpoint {endpoint}"
        );
    }
}

#[test]
fn api_only_build_excludes_remote_modules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main = fs::read_to_string(root.join("src/main.rs")).unwrap();
    let config = fs::read_to_string(root.join("src/config.rs")).unwrap();

    for removed_module in [
        "bridge_core",
        "bridge_inbound_attachments",
        "jwt_utils",
        "remote_sdk",
        "transports",
        "work_secret",
    ] {
        assert!(
            !main.contains(&format!("mod {removed_module};")),
            "removed remote module reintroduced: {removed_module}"
        );
    }
    for removed_file in [
        "src/bridge_core.rs",
        "src/bridge_inbound_attachments.rs",
        "src/jwt_utils.rs",
        "src/remote_sdk.rs",
        "src/work_secret.rs",
        "src/transports/mod.rs",
    ] {
        assert!(
            !root.join(removed_file).exists(),
            "removed remote file reintroduced: {removed_file}"
        );
    }
    assert!(!main.contains("--remote-session-id"));
    assert!(!main.contains("--remote-url"));
    assert!(config.contains("default_provider: \"openai\".into()"));
    assert!(!config.contains("default_provider: \"local\".into()"));
    assert!(!config.contains("qwen2.5-coder:7b"));
}
