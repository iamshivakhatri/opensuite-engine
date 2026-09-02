//! DOCX semantics built on Open Packaging Conventions infrastructure.

/// Identifies this architectural layer before DOCX support exists.
pub const LAYER: &str = "docx";

/// The package layer used by this crate.
pub const PACKAGE_LAYER: &str = opensuite_opc::LAYER;

#[cfg(test)]
mod tests {
    use super::{LAYER, PACKAGE_LAYER};

    #[test]
    fn depends_on_the_opc_layer() {
        assert_eq!((LAYER, PACKAGE_LAYER), ("docx", "opc"));
    }
}
