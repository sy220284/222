//! Hand port of 2.1.88 src/utils/generatedFiles.ts.
//! These rules affect attribution metrics only. They never deny a file edit.
use std::sync::OnceLock;

use regex::Regex;

const EXCLUDED_FILENAMES: &[&str] = &[
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "bun.lockb",
    "bun.lock",
    "composer.lock",
    "gemfile.lock",
    "cargo.lock",
    "poetry.lock",
    "pipfile.lock",
    "shrinkwrap.json",
    "npm-shrinkwrap.json",
];

const EXCLUDED_EXTENSIONS: &[&str] = &[
    ".lock",
    ".min.js",
    ".min.css",
    ".min.html",
    ".bundle.js",
    ".bundle.css",
    ".generated.ts",
    ".generated.js",
    ".d.ts",
];

const EXCLUDED_DIRECTORIES: &[&str] = &[
    "/dist/",
    "/build/",
    "/out/",
    "/output/",
    "/node_modules/",
    "/vendor/",
    "/vendored/",
    "/third_party/",
    "/third-party/",
    "/external/",
    "/.next/",
    "/.nuxt/",
    "/.svelte-kit/",
    "/coverage/",
    "/__pycache__/",
    "/.tox/",
    "/venv/",
    "/.venv/",
    "/target/release/",
    "/target/debug/",
];

fn filename_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            r"^.*\.min\.[a-z]+$",
            r"^.*-min\.[a-z]+$",
            r"^.*\.bundle\.[a-z]+$",
            r"^.*\.generated\.[a-z]+$",
            r"^.*\.gen\.[a-z]+$",
            r"^.*\.auto\.[a-z]+$",
            r"^.*_generated\.[a-z]+$",
            r"^.*_gen\.[a-z]+$",
            r"^.*\.pb\.(go|js|ts|py|rb)$",
            r"^.*_pb2?\.py$",
            r"^.*\.pb\.h$",
            r"^.*\.grpc\.[a-z]+$",
            r"^.*\.swagger\.[a-z]+$",
            r"^.*\.openapi\.[a-z]+$",
        ]
        .into_iter()
        .map(|pattern| Regex::new(&format!("(?i){pattern}")).expect("fixed source pattern"))
        .collect()
    })
}

pub fn is_generated_file(file_path: &str) -> bool {
    // Match Node basename/posix checks for slash paths, including Windows
    // filenames when running tests on a non-Windows host.
    let path = file_path.replace('\\', "/");
    let normalized_path = format!("/{}", path.trim_start_matches('/'));
    let filename = path.rsplit('/').next().unwrap_or("").to_lowercase();

    if EXCLUDED_FILENAMES.contains(&filename.as_str()) {
        return true;
    }
    // Node path.extname returns only the final suffix, e.g. ".js".
    if let Some(dot) = filename.rfind('.') {
        if EXCLUDED_EXTENSIONS.contains(&&filename[dot..]) {
            return true;
        }
    }
    let mut parts = filename.rsplit('.');
    if let (Some(ext), Some(qualifier)) = (parts.next(), parts.next()) {
        let compound = format!(".{qualifier}.{ext}");
        if EXCLUDED_EXTENSIONS.contains(&compound.as_str()) {
            return true;
        }
    }
    if EXCLUDED_DIRECTORIES
        .iter()
        .any(|directory| normalized_path.contains(directory))
    {
        return true;
    }
    filename_patterns()
        .iter()
        .any(|pattern| pattern.is_match(&filename))
}

pub fn filter_generated_files(files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|file| !is_generated_file(file))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_filename_and_compound_extensions() {
        for path in [
            "Cargo.lock", "pnpm-lock.yaml", "src/x.d.ts", "src/code.MIN.JS",
            "src/bundle.bundle.css", "src/foo.generated.ts",
            "proto/account.pb.go", "proto/account_pb2.py",
            "proto/account_pb.py", "proto/service.grpc.ts",
            "schema.openapi.json", "src/A-min.js", "src/a.auto.py",
            "src/a.gen.rs", "src/data.pb.h", "src/foo.swagger.yaml",
        ] {
            assert!(is_generated_file(path), "expected generated: {path}");
        }
        for path in [
            "src/normal.rs", "Cargo.toml", "src/types.ts",
            "src/package-lock.json.bak", "src/production.locked", "src/a_auto.py",
            "src/distinguished/file.rs", "src/a.pb.h.txt",
            "src/README.md",
        ] {
            assert!(!is_generated_file(path), "expected authored: {path}");
        }
    }

    #[test]
    fn directory_rule_requires_complete_path_component() {
        for path in [
            "dist/index.js",
            "nested/node_modules/fixture.js",
            "vendor/lib.rs", "target/release/app",
            "target/debug/app", ".next/static/main.js",
            "nested/third-party/client.rs",
            "nested\\coverage\\stats.txt",
        ] {
            assert!(is_generated_file(path), "{path}");
        }
        assert!(!is_generated_file("distinguished/mod.rs"));
        // The upstream directory match is case-sensitive.
        assert!(!is_generated_file("src/Dist/main.rs"));
    }

    #[test]
    fn filters_without_reordering_or_deduplicating_authored_paths() {
        let paths = vec![
            "src/main.rs".into(), "Cargo.lock".into(), "src/main.rs".into(),
            "dist/run.js".into(), "src/lib.rs".into(),
        ];
        assert_eq!(
            filter_generated_files(&paths),
            ["src/main.rs", "src/main.rs", "src/lib.rs"]
        );
    }
}
