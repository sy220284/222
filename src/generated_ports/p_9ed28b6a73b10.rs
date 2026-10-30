//! Rust exports derived from original: src/hooks/useAfterFirstRender.ts
//!
//! After-first-render startup timing probe: for internal cohorts only, print
//! startup milliseconds to stderr once and exit cleanly. The env contract
//! (USER_TYPE cohort + the exit-after-first-render flag, mapped to the
//! neutral ATLAS_ prefix) and the rounding semantics are preserved.

/// `useAfterFirstRender` effect body. Returns the startup message when the
/// probe fired; the CLI caller prints it to stderr and exits 0.
pub fn after_first_render(uptime_seconds: f64) -> Option<String> {
    let internal = std::env::var("USER_TYPE").ok().as_deref() == Some("ant");
    let exit_flag = crate::env_runtime::is_env_truthy(
        std::env::var("ATLAS_EXIT_AFTER_FIRST_RENDER")
            .ok()
            .as_deref(),
    );
    if internal && exit_flag {
        Some(format!(
            "\nStartup time: {}ms\n",
            (uptime_seconds * 1000.0).round() as i64
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn probe_requires_both_cohort_and_flag() {
        std::env::remove_var("USER_TYPE");
        std::env::remove_var("ATLAS_EXIT_AFTER_FIRST_RENDER");
        assert!(super::after_first_render(1.2345).is_none());
        std::env::set_var("USER_TYPE", "ant");
        assert!(super::after_first_render(1.2345).is_none());
        std::env::set_var("ATLAS_EXIT_AFTER_FIRST_RENDER", "1");
        let message = super::after_first_render(1.2345).unwrap();
        assert!(message.contains("Startup time: 1235ms"));
        std::env::remove_var("USER_TYPE");
        std::env::remove_var("ATLAS_EXIT_AFTER_FIRST_RENDER");
    }
}
