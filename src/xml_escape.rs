//! XML text and quoted attribute escaping, preserved from original src/utils/xml.ts.
//! Escape ampersands first to prevent nested / double escaped substitutions.

pub fn escape_xml(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn escape_xml_attr(value: &str) -> String {
    escape_xml(value).replace('"', "&quot;").replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_escaping_matches_original_order_and_does_not_escape_quotes() {
        assert_eq!(escape_xml(""), "");
        assert_eq!(escape_xml("normal"), "normal");
        assert_eq!(escape_xml("a&b<c>d"), "a&amp;b&lt;c&gt;d");
        assert_eq!(escape_xml(r#""'&<>"#), r#""'&amp;&lt;&gt;"#);
        assert_eq!(escape_xml("😀\n&"), "😀\n&amp;");
        // Escaping is for a single interpolation pass, not idempotence.
        assert_eq!(escape_xml("&amp;"), "&amp;amp;");
    }
    #[test]
    fn attribute_escaping_handles_both_quote_types_and_untrusted_values() {
        assert_eq!(escape_xml_attr(r#""'&<>"#),
            "&quot;&apos;&amp;&lt;&gt;");
        assert_eq!(escape_xml_attr("it’s 'safe'"), "it’s &apos;safe&apos;");
        assert_eq!(escape_xml_attr("\t\n"), "\t\n");
    }
}
