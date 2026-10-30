//! Local OS browser/file opener based on 2.1.88 src/utils/browser.ts.
//! URL validation is performed before spawning an external process and
//! failed launch commands return false, matching the original API.
use reqwest::Url;
use tokio::process::Command;

pub fn validate_browser_url(uri: &str) -> anyhow::Result<Url> {
    let url = Url::parse(uri)?;
    if !matches!(url.scheme(), "http" | "https") {
        anyhow::bail!("Invalid URL protocol: must use http:// or https://");
    }
    Ok(url)
}

async fn launch(command: &str, args: &[&str]) -> bool {
    let mut process = Command::new(command);
    process.args(args);
    crate::exec_file::execute(
        process,
        crate::exec_file::Options {
            preserve_output_on_error: false,
            ..Default::default()
        },
    )
    .await
    .success()
}

pub async fn open_path(path: &str) -> bool {
    #[cfg(windows)]
    {
        launch("explorer", &[path]).await
    }
    #[cfg(target_os = "macos")]
    {
        launch("open", &[path]).await
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        launch("xdg-open", &[path]).await
    }
}

pub async fn open_browser(uri: &str) -> bool {
    // The original validates with new URL(uri), then passes the original
    // string to the browser. Url::as_str() canonicalizes some inputs and can
    // silently change the address the user asked to open.
    if validate_browser_url(uri).is_err() {
        return false;
    }
    let preferred = std::env::var("BROWSER").ok().filter(|s| !s.is_empty());
    #[cfg(windows)]
    {
        if let Some(browser) = preferred {
            // Preserve the original Windows argument quoting, without
            // running a shell that could execute URI query parameters.
            return launch(&browser, &[&format!("\"{uri}\"")]).await;
        }
        launch("rundll32", &["url,OpenURL", uri]).await
    }
    #[cfg(target_os = "macos")]
    {
        launch(preferred.as_deref().unwrap_or("open"), &[uri]).await
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        launch(preferred.as_deref().unwrap_or("xdg-open"), &[uri]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_uses_the_real_argv_process_and_reports_exit_failure() {
        assert!(launch("sh", &["-c", "exit 0"]).await);
        assert!(!launch("sh", &["-c", "exit 23"]).await);
        assert!(!launch("/nonexistent/atlas-browser", &[]).await);
    }

    #[test]
    fn accepts_only_http_and_https_without_stripping_uri_parameters() {
        for value in [
            "file:///tmp/test",
            "javascript:alert(1)",
            "data:text/html,hello",
            "mailto:user@example.org",
            "not-a-url",
        ] {
            assert!(validate_browser_url(value).is_err());
        }
        let input = "https://example.org/search?q=a%26b#section";
        assert_eq!(validate_browser_url(input).unwrap().to_string(), input);
        assert!(validate_browser_url("http://localhost:8080/").is_ok());
    }
}
