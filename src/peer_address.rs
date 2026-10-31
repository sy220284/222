//! The original 2.1.88 peer-address routing rule in an independent Rust module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerScheme {
    Uds,
    Bridge,
    Other,
}
pub fn parse_peer_address(value: &str) -> (PeerScheme, &str) {
    if let Some(target) = value.strip_prefix("uds:") {
        return (PeerScheme::Uds, target);
    }
    if let Some(target) = value.strip_prefix("bridge:") {
        return (PeerScheme::Bridge, target);
    }
    if value.starts_with('/') {
        return (PeerScheme::Uds, value);
    }
    (PeerScheme::Other, value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_prefixes_and_legacy_paths_preserve_targets() {
        assert_eq!(parse_peer_address("uds:/tmp/a"), (PeerScheme::Uds, "/tmp/a"));
        assert_eq!(parse_peer_address("bridge:session"), (PeerScheme::Bridge, "session"));
        assert_eq!(parse_peer_address("/tmp/a"), (PeerScheme::Uds, "/tmp/a"));
        assert_eq!(parse_peer_address(" agent"), (PeerScheme::Other, " agent"));
        assert_eq!(parse_peer_address("uds:"), (PeerScheme::Uds, ""));
    }
}
