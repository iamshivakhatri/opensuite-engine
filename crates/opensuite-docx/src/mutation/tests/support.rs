pub(super) use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    io::{Read, Write},
    sync::atomic::{AtomicUsize, Ordering},
};

#[allow(unused_imports)]
pub(super) use opensuite_protocol::{
    ContentControlTarget, CreateTable, DeletePageBreak, DeleteParagraph, DeletePicture,
    HeaderFooterKind, InsertPageBreak, InsertParagraph, InsertParagraphAfter, InsertPicture,
    InsertTableColumnAfter, InsertTableRowAfter, InsertTableRowsAfter, InspectDocx,
    InspectDocxContent, InspectDocxFocus, PageMargins, PageOrientation, PaperSize,
    ParagraphAlignment, ParagraphFormattingPatch, ParagraphListKind, ParagraphPlacement,
    PictureSizeChange, PropertyPatch, ReplaceText, SetContentControlText, SetHeaderFooterText,
    SetHyperlink, SetPageSetup, SetParagraphFormatting, SetParagraphStyle, SetParagraphsList,
    SetPictureSize, SetTableCellText, SetTableCellsText, SetTextFormatting, TableCellTarget,
    TableCellTextUpdate, TableRowTarget, TableTarget, TextFormattingPatch, TextTarget,
};
pub(super) use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

pub(super) use super::super::*;

pub(super) const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
pub(super) const OFFICE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
pub(super) static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

pub(super) fn imported_numbering_fixture() -> Vec<u8> {
    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    for (name, bytes) in [
        (
            "[Content_Types].xml",
            format!(
                r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/numbering.xml" ContentType="{NUMBERING_CONTENT_TYPE}"/></Types>"#
            ),
        ),
        (
            "_rels/.rels",
            format!(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{OFFICE}" Target="word/document.xml"/></Relationships>"#
            ),
        ),
        (
            "word/_rels/document.xml.rels",
            format!(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId77" Type="{NUMBERING_RELATIONSHIP_TYPE}" Target="numbering.xml"/><Relationship Id="rId99" Type="urn:example:preserve" Target="custom.xml"/></Relationships>"#
            ),
        ),
        (
            "word/document.xml",
            format!(
                r#"<w:document xmlns:w="{WORD}"><w:body><w:p><w:pPr><w:pStyle w:val="HeadingOne"/><w:keepNext/><w:jc w:val="center"/><w:spacing w:before="120" w:after="240"/><w:ind w:left="360"/></w:pPr><w:r><w:t>First</w:t></w:r></w:p><w:p><w:r><w:t>Second</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="42"/></w:numPr></w:pPr><w:r><w:t>Existing</w:t></w:r></w:p></w:body></w:document>"#
            ),
        ),
        (
            "word/styles.xml",
            format!(
                r#"<w:styles xmlns:w="{WORD}"><w:style w:type="paragraph" w:styleId="HeadingOne"><w:name w:val="Heading 1"/></w:style></w:styles>"#
            ),
        ),
        (
            "word/numbering.xml",
            format!(
                r#"<w:numbering xmlns:w="{WORD}"><w:abstractNum w:abstractNumId="77"><w:nsid w:val="ABCD1234"/><w:multiLevelType w:val="singleLevel"/><w:legacy w:legacy="preserve-me"/><w:lvl w:ilvl="0"><w:start w:val="5"/><w:numFmt w:val="upperRoman"/><w:lvlText w:val="%1)"/></w:lvl></w:abstractNum><w:num w:numId="42"><w:abstractNumId w:val="77"/></w:num></w:numbering>"#
            ),
        ),
        ("word/custom.xml", "<custom/>".to_owned()),
    ] {
        zip.start_file(name, options).unwrap();
        zip.write_all(bytes.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

pub(super) fn path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "opensuite-mutation-{name}-{}-{}.docx",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ))
}

pub(super) fn valid_picture_fixture() -> std::path::PathBuf {
    let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/images.docx");
    let output = path("picture-input");
    let mut input = ZipArchive::new(fs::File::open(source).unwrap()).unwrap();
    let mut writer = ZipWriter::new(fs::File::create(&output).unwrap());
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().to_owned();
        if entry.is_dir() {
            writer
                .add_directory(name, SimpleFileOptions::default())
                .unwrap();
        } else {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&bytes).unwrap();
        }
    }
    writer
        .start_file("media/missing.png", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"missing fixture image").unwrap();
    writer.finish().unwrap();
    output
}

pub(super) fn google_docs_table_fixture() -> &'static [u8] {
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/google-docs-table.docx"
    ))
}

pub(super) fn payloads(path: &std::path::Path) -> BTreeMap<String, Vec<u8>> {
    let mut archive = ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    (0..archive.len())
        .filter_map(|index| {
            let mut entry = archive.by_index(index).unwrap();
            (!entry.is_dir()).then(|| {
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).unwrap();
                (entry.name().to_owned(), bytes)
            })
        })
        .collect()
}

pub(super) fn picture_fixture(pictures: &[(&str, &str, &str)]) -> std::path::PathBuf {
    let output = path("picture-targets");
    let mut writer = ZipWriter::new(fs::File::create(&output).unwrap());
    let options = SimpleFileOptions::default();
    writer.start_file("[Content_Types].xml", options).unwrap();
    writer.write_all(b"<Types><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"png\" ContentType=\"image/png\"/><Override PartName=\"/custom/main.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>").unwrap();
    writer.start_file("_rels/.rels", options).unwrap();
    writer.write_all(format!("<Relationships><Relationship Id=\"rId1\" Type=\"{OFFICE}\" Target=\"custom/main.xml\"/></Relationships>").as_bytes()).unwrap();
    writer.start_file("custom/main.xml", options).unwrap();
    let mut body = String::new();
    for (index, (name, relationship, _)) in pictures.iter().enumerate() {
        write!(body, "<w:p><w:r><w:drawing><wp:inline><wp:extent cx=\"1\" cy=\"2\"/><wp:docPr id=\"{}\" name=\"{}\"/><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed=\"{}\"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>", index + 1, name, relationship).unwrap();
    }
    writer.write_all(format!("<w:document xmlns:w=\"{WORD}\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><w:body>{body}</w:body></w:document>").as_bytes()).unwrap();
    writer
        .start_file("custom/_rels/main.xml.rels", options)
        .unwrap();
    let mut relationships = String::new();
    for (_, relationship, target) in pictures {
        write!(relationships, "<Relationship Id=\"{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"{}\"/>", relationship, target).unwrap();
    }
    writer
        .write_all(format!("<Relationships>{relationships}</Relationships>").as_bytes())
        .unwrap();
    let mut written = std::collections::BTreeSet::new();
    for (_, _, target) in pictures {
        let name = target.trim_start_matches("../");
        if written.insert(name) {
            writer.start_file(name, options).unwrap();
            writer.write_all(b"original image").unwrap();
        }
    }
    writer.finish().unwrap();
    output
}

pub(super) fn fixture() -> std::path::PathBuf {
    let path = path("input");
    let file = fs::File::create(&path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(b"<Types><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>")
        .unwrap();
    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(format!("<Relationships><Relationship Id=\"rId1\" Type=\"{OFFICE}\" Target=\"word/document.xml\"/></Relationships>").as_bytes()).unwrap();
    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(format!("<w:document xmlns:w=\"{WORD}\"><w:body><w:p><w:r><w:t>OLD UNIQUE TEXT</w:t></w:r></w:p><w:p><w:r><w:t>Duplicate</w:t></w:r><w:r><w:t>Duplicate</w:t></w:r></w:p><w:p><w:r><w:t>Cross </w:t></w:r><w:r><w:t>run</w:t></w:r></w:p><w:p><w:r><w:t>Styled </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>text</w:t></w:r></w:p><w:p><w:r><w:t>Linked </w:t></w:r><w:hyperlink><w:r><w:t>text</w:t></w:r></w:hyperlink></w:p><w:p><w:r><w:t>FY2026 Revenue </w:t></w:r><w:r><w:t>was $10M according</w:t></w:r></w:p><w:p><w:r><w:t xml:space=\"preserve\"> spaced </w:t></w:r></w:p><w:p><w:del><w:r><w:delText>OLD DELETED</w:delText></w:r></w:del><w:ins><w:r><w:t>NEW INSERTED</w:t></w:r></w:ins></w:p></w:body></w:document>").as_bytes()).unwrap();
    zip.start_file("word/media/image.bin", options).unwrap();
    zip.write_all(&[1, 2, 3]).unwrap();
    zip.finish().unwrap();
    path
}

pub(super) fn table_fixture(document: &str) -> std::path::PathBuf {
    let path = path("table-input");
    let file = fs::File::create(&path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(b"<Types><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>")
        .unwrap();
    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(format!("<Relationships><Relationship Id=\"rId1\" Type=\"{OFFICE}\" Target=\"word/document.xml\"/></Relationships>").as_bytes()).unwrap();
    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(document.as_bytes()).unwrap();
    zip.start_file("word/media/image.bin", options).unwrap();
    zip.write_all(&[1, 2, 3]).unwrap();
    zip.finish().unwrap();
    path
}

pub(super) fn styled_fixture(document: &str) -> std::path::PathBuf {
    let path = path("style-input");
    let file = fs::File::create(&path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(b"<Types><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>")
        .unwrap();
    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(format!("<Relationships><Relationship Id=\"rId1\" Type=\"{OFFICE}\" Target=\"word/document.xml\"/></Relationships>").as_bytes()).unwrap();
    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(document.as_bytes()).unwrap();
    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    zip.write_all(b"<Relationships><Relationship Id=\"rIdStyles\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>").unwrap();
    zip.start_file("word/styles.xml", options).unwrap();
    zip.write_all(format!("<w:styles xmlns:w=\"{WORD}\"><w:style w:type=\"paragraph\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style><w:style w:type=\"paragraph\" w:styleId=\"HeadingOne\"><w:name w:val=\"Heading 1\"/></w:style><w:style w:type=\"character\" w:styleId=\"Emphasis\"><w:name w:val=\"Emphasis\"/></w:style></w:styles>").as_bytes()).unwrap();
    zip.start_file("word/media/image.bin", options).unwrap();
    zip.write_all(&[1, 2, 3]).unwrap();
    zip.finish().unwrap();
    path
}

pub(super) fn table_operation(
    row_label: &str,
    column_header: &str,
    expected: &str,
    replacement: &str,
) -> SetTableCellText {
    SetTableCellText {
        target: TableCellTarget {
            row_label: row_label.to_owned(),
            column_header: column_header.to_owned(),
            occurrence: None,
            handle: None,
        },
        expected_current_text: expected.to_owned(),
        replacement: replacement.to_owned(),
        base_revision: Some("caller-version-7".to_owned()),
    }
}

pub(super) fn table_execute(
    input: &Path,
    output: &Path,
    operation: &SetTableCellText,
) -> OperationResult {
    let package = Package::open(input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    set_table_cell_text(&package, &main, &source, operation, output)
}

pub(super) fn cells_execute(
    input: &Path,
    operation: &SetTableCellsText,
) -> Result<Vec<u8>, OperationResult> {
    let package = Package::open(input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    set_table_cells_text_to_vec(&package, &main, &source, operation)
}

pub(super) fn control_operation(
    tag: Option<&str>,
    alias: Option<&str>,
    expected: &str,
    replacement: &str,
) -> SetContentControlText {
    SetContentControlText {
        target: ContentControlTarget {
            tag: tag.map(str::to_owned),
            alias: alias.map(str::to_owned),
            occurrence: None,
        },
        expected_current_text: expected.to_owned(),
        replacement: replacement.to_owned(),
        base_revision: Some("caller-version-7".to_owned()),
    }
}

pub(super) fn control_execute(
    input: &Path,
    output: &Path,
    operation: &SetContentControlText,
) -> OperationResult {
    let package = Package::open(input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    set_content_control_text(&package, &main, &source, operation, output)
}

pub(super) fn operation(target: &str, expected: &str, replacement: &str) -> ReplaceText {
    ReplaceText {
        target: TextTarget {
            text: target.to_owned(),
            occurrence: None,
        },
        expected_current_text: expected.to_owned(),
        replacement: replacement.to_owned(),
        base_revision: Some("caller-version-7".to_owned()),
    }
}

pub(super) fn execute(input: &Path, output: &Path, operation: &ReplaceText) -> OperationResult {
    let package = Package::open(input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    replace_text(&package, &main, &source, operation, output)
}

pub(super) fn insert_operation(anchor: &str, text: &str) -> InsertParagraphAfter {
    InsertParagraphAfter {
        anchor: TextTarget {
            text: anchor.to_owned(),
            occurrence: None,
        },
        text: text.to_owned(),
        base_revision: Some("caller-version-7".to_owned()),
    }
}

pub(super) fn insert_execute(
    input: &Path,
    output: &Path,
    operation: &InsertParagraphAfter,
) -> OperationResult {
    let package = Package::open(input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    insert_paragraph_after(&package, &main, &source, operation, output)
}

pub(super) fn delete_operation(target: &str) -> DeleteParagraph {
    DeleteParagraph {
        target: TextTarget {
            text: target.to_owned(),
            occurrence: None,
        },
        base_revision: Some("caller-version-7".to_owned()),
    }
}

pub(super) fn delete_execute(
    input: &Path,
    output: &Path,
    operation: &DeleteParagraph,
) -> OperationResult {
    let package = Package::open(input).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    delete_paragraph(&package, &main, &source, operation, output)
}

pub(super) fn entry(path: &Path, name: &str) -> Vec<u8> {
    let file = fs::File::open(path).unwrap();
    let mut zip = ZipArchive::new(file).unwrap();
    let mut value = Vec::new();
    std::io::Read::read_to_end(&mut zip.by_name(name).unwrap(), &mut value).unwrap();
    value
}

pub(super) fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 2, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

pub(super) fn jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = vec![0xff, 0xd8, 0xff, 0xc0, 0, 17, 8];
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&[3, 1, 17, 0, 2, 17, 0, 3, 17, 0, 0xff, 0xd9]);
    bytes
}
