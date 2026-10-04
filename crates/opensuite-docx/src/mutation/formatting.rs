pub(super) use super::common::element_insertion as ppr_insertion;
use super::*;

mod paragraph;
mod text;

pub use paragraph::*;
pub(super) use paragraph::{new_formatting_children, paragraph_property_patches, simple_property};
pub(super) use text::table_cell_text_formatting_patches;
pub use text::*;
pub(super) use text::{
    new_text_formatting_children, run_property_patches, validate_text_formatting,
};

pub(super) fn edit_attribute(mut tag: String, key: &str, value: Option<&str>) -> String {
    // Read attribute boundaries so single quotes, whitespace around =, and values
    // containing another attribute's name remain safe in imported XML.
    let bytes = tag.as_bytes();
    let mut at = 1;
    while at < bytes.len() && !bytes[at].is_ascii_whitespace() {
        at += 1;
    }
    let mut found = None;
    while at < bytes.len() {
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let start = at;
        while at < bytes.len() && !bytes[at].is_ascii_whitespace() && !b"=/>".contains(&bytes[at]) {
            at += 1;
        }
        let name_end = at;
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        if bytes.get(at) != Some(&b'=') {
            break;
        }
        at += 1;
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let Some(&quote) = bytes.get(at) else {
            break;
        };
        if quote != b'\'' && quote != b'"' {
            break;
        }
        at += 1;
        while at < bytes.len() && bytes[at] != quote {
            at += 1;
        }
        at += 1;
        if &tag[start..name_end] == key {
            found = Some(start..at);
            break;
        }
    }
    if let Some(range) = found {
        tag.replace_range(
            range,
            &value
                .map(|value| format!("{key}=\"{value}\""))
                .unwrap_or_default(),
        );
    } else if let Some(value) = value {
        let at = tag
            .rfind("/>")
            .or_else(|| tag.rfind('>'))
            .unwrap_or(tag.len());
        tag.insert_str(at, &format!(" {key}=\"{value}\""));
    }
    tag
}

pub(super) fn matches_scalar<T: PartialEq, U>(
    actual: &Option<T>,
    patch: Option<&PropertyPatch<U>>,
    map: impl Fn(U) -> T,
) -> bool
where
    U: Copy,
{
    match patch {
        None => true,
        Some(PropertyPatch::Clear) => actual.is_none(),
        Some(PropertyPatch::Set(value)) => actual.as_ref() == Some(&map(*value)),
    }
}
