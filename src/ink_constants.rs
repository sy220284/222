/// Terminal animation frame duration, milliseconds.
pub const FRAME_INTERVAL_MS: u64 = 16;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_render_frame_interval() {
        assert_eq!(FRAME_INTERVAL_MS, 16);
    }
}
