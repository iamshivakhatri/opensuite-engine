use super::*;
#[test]
fn rejects_invalid_handles_and_reports_invalid_packages() {
    for handle in [
        "",
        "p0",
        "d0:123",
        "image:/word/document.xml:0:0000000000000000",
        "image:/word/styles.xml:0:0",
    ] {
        assert!(read_docx_embedded_image(crate::create_blank_docx(), handle).is_err());
    }
    assert!(!inspect_docx_images(vec![1, 2, 3], 0, 20).ok);
    let empty = inspect_docx_images(crate::create_blank_docx(), 0, 20);
    assert!(empty.ok);
    assert_eq!(empty.total, 0);
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes.extend([8, 2, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}
fn fixture() -> (Vec<u8>, Vec<u8>) {
    let image = png(240, 120);
    let result = crate::execute_docx_insert_picture(
        crate::create_blank_docx(),
        &opensuite_protocol::InsertPicture {
            image_bytes: image.clone(),
            placement: opensuite_protocol::ParagraphPlacement::End.into(),
            auto_fit: false,
            alignment: None,
            size: None,
            layout: None,
            alt_text: Some("Native image".into()),
            base_revision: None,
        },
    );
    (result.output_artifact.unwrap(), image)
}
fn change_part(bytes: &[u8], name: &str, edit: impl FnOnce(Vec<u8>) -> Vec<u8>) -> Vec<u8> {
    let package = Package::from_bytes(bytes.to_vec()).unwrap();
    let part = package
        .part(&opensuite_opc::PartName::parse(name).unwrap())
        .unwrap();
    package
        .write_replaced_part_to_vec(&part, &edit(package.read_part(&part).unwrap()))
        .unwrap()
}
#[test]
fn returns_original_bytes_and_stable_handles_without_modifying_source() {
    let (bytes, original_image) = fixture();
    let facts = inspect_docx_images(bytes.clone(), 0, 20);
    assert!(facts.ok);
    assert_eq!(facts.total, 1);
    let record = &facts.images[0];
    assert_eq!(record.format, Some("png"));
    assert_eq!(record.intrinsic_width_px, Some(240));
    assert_eq!(record.display_width_emu, Some(2_286_000));
    assert_eq!(record.alt_text.as_deref(), Some("Native image"));
    let handle = record.handle.as_ref().unwrap();
    assert_eq!(
        read_docx_embedded_image(bytes.clone(), handle).unwrap(),
        original_image
    );
    assert_eq!(
        inspect_docx_images(bytes.clone(), 0, 20).images[0]
            .handle
            .as_ref(),
        Some(handle)
    );
    let package = Package::from_bytes(bytes.clone()).unwrap();
    let reopened = package
        .write_replaced_part_to_vec(
            &package.main_office_document().unwrap(),
            &package
                .read_part(&package.main_office_document().unwrap())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        read_docx_embedded_image(reopened, handle).unwrap(),
        original_image
    );
    let page = inspect_docx_images(bytes, 1, 1);
    assert_eq!(page.total, 1);
    assert!(!page.has_more);
    assert!(page.images.is_empty());
}
#[test]
fn source_relationship_and_media_changes_invalidate_handles() {
    let (bytes, _) = fixture();
    let handle = inspect_docx_images(bytes.clone(), 0, 20).images[0]
        .handle
        .clone()
        .unwrap();
    for (part, from, to) in [
        ("/word/document.xml", "Picture 1", "Different picture"),
        (
            "/word/_rels/document.xml.rels",
            "<Relationships ",
            "<Relationships changed=\"yes\" ",
        ),
    ] {
        let changed = change_part(&bytes, part, |v| {
            String::from_utf8(v).unwrap().replace(from, to).into_bytes()
        });
        assert_eq!(
            read_docx_embedded_image(changed, &handle)
                .unwrap_err()
                .diagnostics[0]
                .code,
            "PRECONDITION_FAILED"
        );
    }
    let changed = change_part(&bytes, "/word/media/image1.png", |_| png(120, 60));
    assert_eq!(
        read_docx_embedded_image(changed, &handle)
            .unwrap_err()
            .diagnostics[0]
            .code,
        "PRECONDITION_FAILED"
    );
}
#[test]
fn bounds_payload_before_reading_and_rejects_linked_or_malformed_relationships() {
    let (bytes, _) = fixture();
    let handle = inspect_docx_images(bytes.clone(), 0, 20).images[0]
        .handle
        .clone()
        .unwrap();
    for (part, from, to, code) in [
        ("/word/document.xml", "r:embed=", "r:link=", "LINKED_IMAGE"),
        (
            "/word/document.xml",
            "r:embed=\"rId",
            "r:embed=\"missing-rId",
            "MISSING_IMAGE_RELATIONSHIP",
        ),
        (
            "/word/_rels/document.xml.rels",
            "/relationships/image",
            "/relationships/hyperlink",
            "WRONG_IMAGE_RELATIONSHIP_TYPE",
        ),
    ] {
        let changed = change_part(&bytes, part, |v| {
            String::from_utf8(v).unwrap().replace(from, to).into_bytes()
        });
        let facts = inspect_docx_images(changed.clone(), 0, 20);
        assert!(facts.images[0].handle.is_none());
        assert_eq!(facts.images[0].diagnostics, [code]);
        assert_eq!(
            read_docx_embedded_image(changed, &handle)
                .unwrap_err()
                .diagnostics[0]
                .code,
            code
        );
    }
    for image in [
        vec![0; MAX_EMBEDDED_IMAGE_BYTES as usize + 1],
        png(100_000, 100_000),
    ] {
        let changed = change_part(&bytes, "/word/media/image1.png", |_| image);
        assert_eq!(
            read_docx_embedded_image(changed, &handle)
                .unwrap_err()
                .diagnostics[0]
                .code,
            "IMAGE_SIZE_LIMIT"
        );
    }
}
#[test]
fn retrieves_jpeg_bytes_without_transcoding() {
    let mut image = vec![
        0xff, 0xd8, 0xff, 0xc0, 0, 17, 8, 0, 120, 0, 240, 3, 1, 17, 0, 2, 17, 1, 3, 17, 1, 0xff,
        0xd9,
    ];
    let result = crate::execute_docx_insert_picture(
        crate::create_blank_docx(),
        &opensuite_protocol::InsertPicture {
            image_bytes: image.clone(),
            placement: opensuite_protocol::ParagraphPlacement::End.into(),
            auto_fit: false,
            alignment: None,
            size: None,
            layout: None,
            alt_text: None,
            base_revision: None,
        },
    );
    let bytes = result.output_artifact.unwrap();
    let facts = inspect_docx_images(bytes.clone(), 0, 20);
    assert_eq!(facts.images[0].format, Some("jpeg"));
    assert_eq!(
        read_docx_embedded_image(bytes, facts.images[0].handle.as_ref().unwrap()).unwrap(),
        std::mem::take(&mut image)
    );
}

#[test]
fn refuses_duplicate_relationship_ids_instead_of_selecting_one() {
    let (bytes, _) = fixture();
    let handle = inspect_docx_images(bytes.clone(), 0, 20).images[0]
        .handle
        .clone()
        .unwrap();
    let changed = change_part(&bytes, "/word/_rels/document.xml.rels", |v| {
        let xml = String::from_utf8(v).unwrap();
        let start = xml.rfind("<Relationship ").unwrap();
        let end = start + xml[start..].find("/>").unwrap() + 2;
        xml.replace(
            "</Relationships>",
            &format!("{}</Relationships>", &xml[start..end]),
        )
        .into_bytes()
    });
    assert_eq!(
        read_docx_embedded_image(changed, &handle)
            .unwrap_err()
            .diagnostics[0]
            .code,
        "AMBIGUOUS_IMAGE_RELATIONSHIP"
    );
}
