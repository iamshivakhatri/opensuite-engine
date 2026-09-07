use std::io::{Cursor, Write};

use zip::{ZipWriter, write::SimpleFileOptions};

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#;
const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
const DOCUMENT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>"#;
const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#;
const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial" w:cs="Arial"/><w:sz w:val="22"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="120" w:line="264" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:before="0" w:after="240"/><w:keepNext/></w:pPr><w:rPr><w:b/><w:sz w:val="40"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="Heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:before="240" w:after="120"/><w:keepNext/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="Heading 2"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:before="200" w:after="100"/><w:keepNext/></w:pPr><w:rPr><w:b/><w:sz w:val="28"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="Heading 3"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:before="160" w:after="80"/><w:keepNext/></w:pPr><w:rPr><w:b/><w:sz w:val="24"/></w:rPr></w:style></w:styles>"#;
const SETTINGS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>"#;

/// Creates a small deterministic DOCX package suitable for ordinary authoring.
pub fn create_blank_docx() -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in [
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", ROOT_RELS),
        ("word/document.xml", DOCUMENT),
        ("word/_rels/document.xml.rels", DOCUMENT_RELS),
        ("word/styles.xml", STYLES),
        ("word/settings.xml", SETTINGS),
    ] {
        writer
            .start_file(name, SimpleFileOptions::default())
            .expect("in-memory ZIP write succeeds");
        writer
            .write_all(content.as_bytes())
            .expect("in-memory ZIP write succeeds");
    }
    writer
        .finish()
        .expect("in-memory ZIP finish succeeds")
        .into_inner()
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read};

    use opensuite_opc::{Package, PartName};
    use zip::ZipArchive;

    #[test]
    fn creates_a_deterministic_valid_document_with_authoring_styles() {
        let first = super::create_blank_docx();
        assert_eq!(first, super::create_blank_docx());
        let package = Package::from_bytes(first).unwrap();
        package.verify().unwrap();
        let (main, source) = crate::open_main_source(&package).unwrap();
        assert_eq!(
            crate::DocxDocument::new(&source).unwrap().blocks().count(),
            0
        );
        let styles = crate::load_styles(&package, &main).unwrap().unwrap();
        for name in ["Normal", "Title", "Heading 1", "Heading 2", "Heading 3"] {
            assert!(styles.styles().any(|style| style.name() == Some(name)));
        }
        let styles_part = package
            .read_part(
                &package
                    .part(&PartName::parse("/word/styles.xml").unwrap())
                    .unwrap(),
            )
            .unwrap();
        let xml = String::from_utf8(styles_part).unwrap();
        assert!(xml.contains("w:after=\"120\""));
        assert!(xml.contains("w:line=\"264\""));
        assert!(xml.contains("w:sz w:val=\"32\""));
        let settings = package
            .read_part(
                &package
                    .part(&PartName::parse("/word/settings.xml").unwrap())
                    .unwrap(),
            )
            .unwrap();
        let settings = String::from_utf8(settings).unwrap();
        assert!(settings.contains("w:name=\"compatibilityMode\""));
        assert!(settings.contains("w:val=\"15\""));
        let relationships = package
            .read_part(
                &package
                    .part(&PartName::parse("/word/_rels/document.xml.rels").unwrap())
                    .unwrap(),
            )
            .unwrap();
        assert!(
            String::from_utf8(relationships)
                .unwrap()
                .contains("relationships/settings")
        );
        let mut zip = ZipArchive::new(Cursor::new(super::create_blank_docx())).unwrap();
        let mut content_types = String::new();
        zip.by_name("[Content_Types].xml")
            .unwrap()
            .read_to_string(&mut content_types)
            .unwrap();
        assert!(content_types.contains("/word/settings.xml"));
    }
}
