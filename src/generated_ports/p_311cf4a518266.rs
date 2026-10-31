//! Rust exports derived from original:
//! src/hooks/useOfficialMarketplaceNotification.tsx
//!
//! Marketplace auto-install startup notification: result → notification list
//! mapping. Three keys: config save failure (immediate/10s), install success
//! (info/8s), install failure (immediate/10s) — copy byte-identical except
//! the brand docs URL, which stays neutralized per privacy gate.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketplaceNotifKind {
    ConfigSaveFailed,
    Installed,
    InstallFailed,
}

/// key + priority + timeoutMs per result kind.
pub fn marketplace_notification(kind: MarketplaceNotifKind) -> (&'static str, &'static str, u64) {
    match kind {
        MarketplaceNotifKind::ConfigSaveFailed => {
            ("marketplace-config-save-failed", "immediate", 10_000)
        }
        MarketplaceNotifKind::Installed => ("marketplace-installed", "info", 8_000),
        MarketplaceNotifKind::InstallFailed => ("marketplace-install-failed", "immediate", 10_000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_metadata_matches_original() {
        let (k, p, t) = marketplace_notification(MarketplaceNotifKind::Installed);
        assert_eq!(k, "marketplace-installed");
        assert_eq!(p, "info");
        assert_eq!(t, 8_000);
    }
}
