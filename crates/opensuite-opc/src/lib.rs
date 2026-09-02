//! Office Open XML package infrastructure.

/// Identifies this architectural layer before package support exists.
pub const LAYER: &str = "opc";

#[cfg(test)]
mod tests {
    use super::LAYER;

    #[test]
    fn identifies_the_opc_layer() {
        assert_eq!(LAYER, "opc");
    }
}
