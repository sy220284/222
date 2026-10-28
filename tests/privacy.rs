use std::{fs, path::Path};

fn source_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() { result.extend(source_files(&path)); }
        else if path.extension().and_then(|v| v.to_str()) == Some("rs") {
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
        "clau".to_owned() + "de", "anthro".to_owned() + "pic",
        "tengu".to_owned(), "data".to_owned() + "dog",
        "opentele".to_owned() + "metry", "sentry".to_owned(),
    ];
    for path in files {
        let body = fs::read_to_string(&path).unwrap().to_ascii_lowercase();
        for word in &blocked_names {
            assert!(!body.contains(word), "unwanted identifier {word} in {}", path.display());
        }
    }
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(!manifest.contains("opentelemetry"));
    assert!(!manifest.contains("sentry"));
}

#[test]
fn no_embedded_vendor_telemetry_server_url() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = source_files(&root.join("src")).into_iter()
        .map(|p| fs::read_to_string(p).unwrap()).collect::<Vec<_>>().join("\n");
    let disallowed = ["statsig", "segment.io", "api.segment",
        "sentry.io", "stats.datadoghq"];
    for endpoint in disallowed {
        assert!(!source.contains(endpoint), "unwanted telemetry endpoint {endpoint}");
    }
}
