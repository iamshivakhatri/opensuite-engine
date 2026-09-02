//! Stable boundary between the engine and its consumers.

/// Identifies this architectural layer before the operation protocol exists.
pub const LAYER: &str = "protocol";

#[cfg(test)]
mod tests {
    use super::LAYER;

    #[test]
    fn identifies_the_protocol_layer() {
        assert_eq!(LAYER, "protocol");
    }
}
