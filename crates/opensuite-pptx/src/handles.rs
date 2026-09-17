#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ShapeHandle {
    pub slide_index: usize,
    pub shape_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RunHandle {
    pub shape: ShapeHandle,
    pub paragraph_index: usize,
    pub run_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParagraphHandle {
    pub shape: ShapeHandle,
    pub paragraph_index: usize,
}

pub(crate) fn parse_paragraph_handle(value: &str) -> Option<ParagraphHandle> {
    let mut parts = value.split(':');
    let parse = |prefix: &str, value: Option<&str>| value?.strip_prefix(prefix)?.parse().ok();
    let target = ParagraphHandle {
        shape: ShapeHandle {
            slide_index: parse("s", parts.next())?,
            shape_index: parse("sh", parts.next())?,
        },
        paragraph_index: parse("p", parts.next())?,
    };
    parts.next().is_none().then_some(target)
}

pub(crate) fn parse_shape_handle(value: &str) -> Option<ShapeHandle> {
    let mut parts = value.split(':');
    let parse = |prefix: &str, value: Option<&str>| value?.strip_prefix(prefix)?.parse().ok();
    let target = ShapeHandle {
        slide_index: parse("s", parts.next())?,
        shape_index: parse("sh", parts.next())?,
    };
    parts.next().is_none().then_some(target)
}

pub(crate) fn parse_run_handle(value: &str) -> Option<RunHandle> {
    let mut parts = value.split(':');
    let parse = |prefix: &str, value: Option<&str>| value?.strip_prefix(prefix)?.parse().ok();
    let target = RunHandle {
        shape: ShapeHandle {
            slide_index: parse("s", parts.next())?,
            shape_index: parse("sh", parts.next())?,
        },
        paragraph_index: parse("p", parts.next())?,
        run_index: parse("r", parts.next())?,
    };
    parts.next().is_none().then_some(target)
}
