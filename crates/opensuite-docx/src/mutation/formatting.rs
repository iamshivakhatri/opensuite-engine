use super::*;

mod paragraph;
mod text;

pub(super) use paragraph::simple_property;
pub use paragraph::*;
pub use text::*;

pub(super) fn edit_attribute(mut tag: String, key: &str, value: Option<&str>) -> String {
    let needle = format!("{key}=\"");
    if let Some(start) = tag.find(&needle) {
        let end = start + needle.len() + tag[start + needle.len()..].find('"').unwrap_or(0) + 1;
        if let Some(value) = value {
            tag.replace_range(start..end, &format!("{key}=\"{value}\""));
        } else {
            let begin = tag[..start].rfind(char::is_whitespace).unwrap_or(start);
            tag.replace_range(begin..end, "");
        }
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
