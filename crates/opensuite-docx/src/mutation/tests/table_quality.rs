use super::super::*;
use opensuite_protocol::TableCellShadingUpdate;

#[test]
fn sets_widths_and_header_shading_without_rewriting_other_properties() {
    let package = Package::from_bytes(crate::create_blank_docx()).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = create_table_to_vec(
        &package,
        &main,
        &source,
        &CreateTable {
            rows: vec![
                vec!["Name".into(), "Notes".into()],
                vec!["A".into(), "Long value".into()],
            ],
            placement: ParagraphPlacement::End,
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let table = TableTarget {
        header_cells: vec!["Name".into(), "Notes".into()],
        occurrence: None,
        handle: None,
    };
    let bytes = set_table_column_widths_to_vec(
        &package,
        &main,
        &source,
        &SetTableColumnWidths {
            table: table.clone(),
            widths_twips: vec![2400, 6960],
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let bytes = set_table_cell_shading_to_vec(
        &package,
        &main,
        &source,
        &SetTableCellShading {
            table: table.clone(),
            updates: vec![
                TableCellShadingUpdate {
                    target: TableCellTarget {
                        row_label: String::new(),
                        column_header: String::new(),
                        occurrence: None,
                        handle: Some("t0:r0:c0".into()),
                    },
                    fill: Some("e9eef5".into()),
                },
                TableCellShadingUpdate {
                    target: TableCellTarget {
                        row_label: String::new(),
                        column_header: String::new(),
                        occurrence: None,
                        handle: Some("t0:r0:c1".into()),
                    },
                    fill: Some("E9EEF5".into()),
                },
            ],
            base_revision: None,
        },
    )
    .unwrap();
    let package = Package::from_bytes(bytes).unwrap();
    package.verify().unwrap();
    let (main, source) = crate::open_main_source(&package).unwrap();
    let xml = String::from_utf8(package.read_part(&main).unwrap()).unwrap();
    assert!(xml.contains(r#"<w:gridCol w:w="2400"/><w:gridCol w:w="6960"/>"#));
    assert_eq!(xml.matches(r#"w:fill="E9EEF5""#).count(), 2);
    for (target, reason_code) in [
        (
            TableCellTarget {
                row_label: "Missing".into(),
                column_header: "Notes".into(),
                occurrence: None,
                handle: None,
            },
            "TABLE_ROW_NOT_FOUND",
        ),
        (
            TableCellTarget {
                row_label: "A".into(),
                column_header: "Missing".into(),
                occurrence: None,
                handle: None,
            },
            "TABLE_COLUMN_NOT_FOUND",
        ),
    ] {
        let result = set_table_cell_shading_to_vec(
            &package,
            &main,
            &source,
            &SetTableCellShading {
                table: table.clone(),
                updates: vec![TableCellShadingUpdate {
                    target,
                    fill: Some("E9EEF5".into()),
                }],
                base_revision: None,
            },
        )
        .unwrap_err();
        assert_eq!(
            result.diagnostics[0].reason_code.as_deref(),
            Some(reason_code)
        );
    }
    assert!(
        set_table_column_widths_to_vec(
            &package,
            &main,
            &source,
            &SetTableColumnWidths {
                table: table.clone(),
                widths_twips: vec![0, 1],
                base_revision: None
            }
        )
        .is_err()
    );
    assert!(
        set_table_column_widths_to_vec(
            &package,
            &main,
            &source,
            &SetTableColumnWidths {
                table: table.clone(),
                widths_twips: vec![3000],
                base_revision: None
            }
        )
        .is_err()
    );
    let cleared = set_table_cell_shading_to_vec(
        &package,
        &main,
        &source,
        &SetTableCellShading {
            table,
            updates: vec![TableCellShadingUpdate {
                target: TableCellTarget {
                    row_label: String::new(),
                    column_header: String::new(),
                    occurrence: None,
                    handle: Some("t0:r0:c0".into()),
                },
                fill: None,
            }],
            base_revision: None,
        },
    )
    .unwrap();
    assert!(
        String::from_utf8(
            Package::from_bytes(cleared)
                .unwrap()
                .read_part(&main)
                .unwrap()
        )
        .unwrap()
        .contains("Long value")
    );
}
