use opensuite_protocol::{Diagnostic, DiagnosticSeverity};

use crate::{PlaceholderOverview, ShapeOverview, handles::parse_shape_handle, inspect_pptx};

const DEFAULT_RESULT_LIMIT: usize = 20;
const MAX_RESULT_LIMIT: usize = 100;
const CONTEXT_LIMIT: usize = 160;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FindPptxText {
    pub text: String,
    pub case_sensitive: bool,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PptxTextSearchResult {
    pub query: String,
    pub matches: Vec<PptxTextMatch>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PptxTextMatch {
    pub slide_handle: String,
    pub slide_index: usize,
    pub shape_handle: String,
    pub shape_name: Option<String>,
    pub placeholder: Option<PlaceholderOverview>,
    pub paragraph_handle: String,
    pub matched_text: String,
    pub context: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub run_handles: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PptxShapeInspection {
    pub shape: Option<ShapeOverview>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Searches supported normal-shape DrawingML run text once in presentation order.
pub fn find_pptx_text(input: Vec<u8>, request: &FindPptxText) -> PptxTextSearchResult {
    if request.text.is_empty() {
        return PptxTextSearchResult {
            query: request.text.clone(),
            matches: Vec::new(),
            diagnostics: vec![Diagnostic::new(
                "INVALID_TEXT_QUERY",
                DiagnosticSeverity::Error,
                "text query must not be empty",
            )],
        };
    }
    let inspection = inspect_pptx(input);
    let Some(overview) = inspection.overview else {
        return PptxTextSearchResult {
            query: request.text.clone(),
            matches: Vec::new(),
            diagnostics: inspection.diagnostics,
        };
    };
    let limit = request
        .limit
        .unwrap_or(DEFAULT_RESULT_LIMIT)
        .max(1)
        .min(MAX_RESULT_LIMIT);
    let mut matches = Vec::new();
    'slides: for slide in overview.slides {
        for shape in slide.shapes {
            let Some(frame) = shape.text_frame else {
                continue;
            };
            for paragraph in frame.paragraphs {
                let text = paragraph
                    .runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>();
                for (start, end) in match_ranges(&text, &request.text, request.case_sensitive) {
                    matches.push(PptxTextMatch {
                        slide_handle: slide.handle.clone(),
                        slide_index: slide.index,
                        shape_handle: shape.handle.clone(),
                        shape_name: shape.name.clone(),
                        placeholder: shape.placeholder.clone(),
                        paragraph_handle: paragraph.handle.clone(),
                        matched_text: text[start..end].to_owned(),
                        context: context(&text, start, end),
                        start_offset: start,
                        end_offset: end,
                        run_handles: touched_runs(&paragraph.runs, start, end),
                    });
                    if matches.len() == limit {
                        break 'slides;
                    }
                }
            }
        }
    }
    PptxTextSearchResult {
        query: request.text.clone(),
        matches,
        diagnostics: Vec::new(),
    }
}

/// Returns one top-level shape's existing semantic inspection detail.
pub fn inspect_pptx_shape(input: Vec<u8>, handle: &str) -> PptxShapeInspection {
    let Some(target) = parse_shape_handle(handle) else {
        return missing_shape();
    };
    let inspection = inspect_pptx(input);
    let Some(overview) = inspection.overview else {
        return PptxShapeInspection {
            shape: None,
            diagnostics: inspection.diagnostics,
        };
    };
    overview
        .slides
        .get(target.slide_index)
        .and_then(|slide| slide.shapes.get(target.shape_index))
        .cloned()
        .map(|shape| PptxShapeInspection {
            shape: Some(shape),
            diagnostics: Vec::new(),
        })
        .unwrap_or_else(missing_shape)
}

fn match_ranges(text: &str, query: &str, case_sensitive: bool) -> Vec<(usize, usize)> {
    if case_sensitive {
        return text
            .match_indices(query)
            .map(|(start, value)| (start, start + value.len()))
            .collect();
    }
    let query_chars = query.chars().collect::<Vec<_>>();
    text.char_indices()
        .filter_map(|(start, _)| {
            let candidate = text[start..]
                .chars()
                .take(query_chars.len())
                .collect::<String>();
            candidate
                .eq_ignore_ascii_case(query)
                .then_some((start, start + candidate.len()))
        })
        .collect()
}

fn touched_runs(runs: &[crate::TextRunOverview], start: usize, end: usize) -> Vec<String> {
    let mut offset = 0usize;
    runs.iter()
        .filter_map(|run| {
            let next = offset + run.text.len();
            let touched = offset < end && start < next;
            offset = next;
            touched.then(|| run.handle.clone())
        })
        .collect()
}

fn context(text: &str, start: usize, end: usize) -> String {
    let prefix = &text[..start];
    let suffix = &text[end..];
    let before = prefix
        .chars()
        .rev()
        .take(CONTEXT_LIMIT / 2)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    let after = suffix
        .chars()
        .take(CONTEXT_LIMIT.saturating_sub(before.chars().count()))
        .collect::<String>();
    format!("{before}{}{}", &text[start..end], after)
}

fn missing_shape() -> PptxShapeInspection {
    PptxShapeInspection {
        shape: None,
        diagnostics: vec![Diagnostic::new(
            "TARGET_NOT_FOUND",
            DiagnosticSeverity::Error,
            "shape handle was not found",
        )],
    }
}
