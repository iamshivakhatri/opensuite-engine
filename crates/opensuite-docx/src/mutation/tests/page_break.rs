use super::super::*;

#[test]
fn inserts_and_deletes_page_breaks_at_every_body_placement() {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = insert_page_break_to_vec(
        &package,
        &main,
        &source,
        &InsertPageBreak {
            placement: ParagraphPlacement::Start,
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = insert_page_break_to_vec(
        &package,
        &main,
        &source,
        &InsertPageBreak {
            placement: ParagraphPlacement::End,
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = insert_page_break_to_vec(
        &package,
        &main,
        &source,
        &InsertPageBreak {
            placement: ParagraphPlacement::Before {
                handle: "b1".to_owned(),
            },
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = insert_page_break_to_vec(
        &package,
        &main,
        &source,
        &InsertPageBreak {
            placement: ParagraphPlacement::After {
                handle: "b1".to_owned(),
            },
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(
        body_block_signatures(&source).unwrap(),
        vec![
            ("page_break".to_owned(), String::new()),
            ("page_break".to_owned(), String::new()),
            ("page_break".to_owned(), String::new()),
            ("page_break".to_owned(), String::new()),
        ]
    );
    let output = delete_page_break_to_vec(
        &package,
        &main,
        &source,
        &DeletePageBreak {
            target: PageBreakTarget {
                handle: "b2".to_owned(),
            },
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(output).unwrap();
    let (_, source) = crate::open_main_source(&package).unwrap();
    assert_eq!(body_block_signatures(&source).unwrap().len(), 3);
}
