use serde_json::{json, Value};
use std::sync::OnceLock;
use tokio::sync::watch;

pub fn empty_usage() -> Value {
    json!({
        "input_tokens":0,"cache_creation_input_tokens":0,
        "cache_read_input_tokens":0,"output_tokens":0,
        "server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},
        "service_tier":"standard",
        "cache_creation":{"ephemeral_1h_input_tokens":0,"ephemeral_5m_input_tokens":0},
        "inference_geo":"","iterations":[],"speed":"standard"
    })
}

fn warning_store()->&'static watch::Sender<bool>{
    static STORE:OnceLock<watch::Sender<bool>>=OnceLock::new();
    STORE.get_or_init(||{
        let(tx,_rx)=watch::channel(false);
        tx
    })
}
pub fn compact_warning_suppressed()->bool{*warning_store().borrow()}
pub fn subscribe_compact_warning()->watch::Receiver<bool>{warning_store().subscribe()}
pub fn suppress_compact_warning(){warning_store().send_replace(true);}
pub fn clear_compact_warning_suppression(){warning_store().send_replace(false);}

/// Generic ASCII-safe name sanitization; the proprietary server prefix
/// special-case is intentionally omitted.
pub fn normalize_mcp_name(name:&str)->String{
    name.encode_utf16().map(|unit|{
        if unit<=0x7f{
            let b=unit as u8;
            if b.is_ascii_alphanumeric()||b==b'_'||b==b'-'{return b as char;}
        }
        '_'
    }).collect()
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn empty_usage_has_the_entire_original_schema(){
        assert_eq!(empty_usage(),json!({
            "input_tokens":0,"cache_creation_input_tokens":0,
            "cache_read_input_tokens":0,"output_tokens":0,
            "server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},
            "service_tier":"standard",
            "cache_creation":{"ephemeral_1h_input_tokens":0,"ephemeral_5m_input_tokens":0},
            "inference_geo":"","iterations":[],"speed":"standard"
        }));
    }
    #[test]
    fn compact_warning_state_and_subscriptions(){
        clear_compact_warning_suppression();
        let mut receiver=subscribe_compact_warning();
        assert!(!compact_warning_suppressed());
        suppress_compact_warning();
        assert!(compact_warning_suppressed());
        assert!(receiver.has_changed().unwrap());
        assert!(*receiver.borrow_and_update());
        clear_compact_warning_suppression();
        assert!(!compact_warning_suppressed());
        assert!(receiver.has_changed().unwrap());
        assert!(!*receiver.borrow_and_update());
    }
    #[test]
    fn generic_mcp_ascii_names_match_reference_nonvendor_behavior(){
        for (input,expected) in [
            ("alpha-Z_09","alpha-Z_09"),
            ("docs.search API","docs_search_API"),
            ("✨","_"),("😀","__"),("a/b","a_b"),("",""),
        ]{
            assert_eq!(normalize_mcp_name(input),expected);
        }
    }
}
