use super::*;

pub(super) fn ensure_safe_deletion(
    source: &SourceDocument,
    regions: &[SourceSpan],
) -> Result<(), OperationResult> {
    super::super::protected_range::ensure_safe_source_ranges(
        source,
        regions,
        "UNSUPPORTED_STRUCTURAL_DELETE",
        false,
    )
}
