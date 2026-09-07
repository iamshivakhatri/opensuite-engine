fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let command = arguments.next();
    let result = if command.as_deref() == Some(std::ffi::OsStr::new("replace-text")) {
        match (
            arguments.next(),
            arguments.next(),
            arguments.next(),
            arguments.next(),
            arguments.next(),
        ) {
            (Some(input), Some(output), Some(target), Some(replacement), None) => {
                replace_text(input, output, target, replacement)
            }
            _ => Err((
                "INVALID_ARGUMENTS",
                "usage: opensuite replace-text <input.docx> <output.docx> <target> <replacement>"
                    .to_owned(),
            )),
        }
    } else if command.as_deref() == Some(std::ffi::OsStr::new("insert-paragraph-after")) {
        insert_paragraph_after(arguments)
    } else if command.as_deref() == Some(std::ffi::OsStr::new("delete-paragraph")) {
        delete_paragraph(arguments)
    } else if command.as_deref() == Some(std::ffi::OsStr::new("set-table-cell-text")) {
        set_table_cell_text(arguments)
    } else if command.as_deref() == Some(std::ffi::OsStr::new("set-content-control-text")) {
        set_content_control_text(arguments)
    } else if command.as_deref() == Some(std::ffi::OsStr::new("set-paragraph-formatting")) {
        set_paragraph_formatting(arguments)
    } else if command.as_deref() == Some(std::ffi::OsStr::new("set-text-formatting")) {
        set_text_formatting(arguments)
    } else if command.as_deref() == Some(std::ffi::OsStr::new("set-paragraph-style")) {
        set_paragraph_style(arguments)
    } else if command.as_deref() == Some(std::ffi::OsStr::new("replace-picture")) {
        replace_picture(arguments)
    } else if command.as_deref() == Some(std::ffi::OsStr::new("inspect-context")) {
        inspect_context(arguments)
    } else {
        match (command, arguments.next(), arguments.next()) {
        (Some(command), None, None) if command == "capabilities" => capabilities(),
        (Some(command), Some(path), None) if command == "inspect" => inspect(path),
        (Some(command), Some(path), None) if command == "inspect-source" => inspect_source(path),
        (Some(command), Some(path), None) if command == "inspect-docx" => inspect_docx(path),
        (Some(command), Some(path), None) if command == "inspect-styles" => inspect_styles(path),
        (Some(command), Some(path), None) if command == "inspect-paragraphs" => inspect_paragraphs(path),
        (Some(command), Some(path), None) if command == "inspect-numbering" => inspect_numbering(path),
        (Some(command), Some(path), None) if command == "inspect-sections" => inspect_sections(path),
        (Some(command), Some(path), None) if command == "inspect-headers-footers" => inspect_headers_footers(path),
        (Some(command), Some(path), None) if command == "inspect-references" => inspect_references(path),
        (Some(command), Some(path), None) if command == "inspect-images" => inspect_images(path),
        (Some(command), Some(path), None) if command == "inspect-fields" => inspect_fields(path),
        (Some(command), Some(path), None) if command == "inspect-content-controls" => inspect_content_controls(path),
        (Some(command), Some(path), None) if command == "inspect-tracked-changes" => inspect_tracked_changes(path),
        (Some(command), Some(path), None) if command == "inspect-comments" => inspect_comments(path),
        (Some(command), Some(path), Some(text)) if command == "find-text" => find_text(path, text),
        (Some(command), Some(path), Some(view)) if command == "inspect-revision-view" => {
            inspect_revision_view(path, view)
        }
        _ => Err((
            "INVALID_ARGUMENTS",
            "usage: opensuite <capabilities|inspect|inspect-source|inspect-docx|inspect-styles|inspect-paragraphs|inspect-numbering|inspect-sections|inspect-headers-footers|inspect-references|inspect-images|inspect-fields|inspect-content-controls|inspect-tracked-changes|inspect-comments> [path-to-office-file] | opensuite <inspect-revision-view|find-text> <path-to-office-file> <current|original|text> | opensuite inspect-context <input.docx> <text> [occurrence] [before] [after] | opensuite insert-paragraph-after <input.docx> <output.docx> <anchor-text> <new-paragraph-text> [occurrence] | opensuite delete-paragraph <input.docx> <output.docx> <target-text> [occurrence] | opensuite set-table-cell-text <input.docx> <output.docx> <row-label> <column-header> <expected-current-text> <replacement> [occurrence] | opensuite set-content-control-text <input.docx> <output.docx> <tag> <expected-current-text> <replacement> [occurrence] | opensuite set-paragraph-formatting <input.docx> <output.docx> <target-text> [--alignment value|--clear-alignment|--space-before twips|--clear-space-before|--space-after twips|--clear-space-after|--line value [--line-rule auto|exact|atLeast]|--clear-line|--left-indent twips|--right-indent twips|--first-line-indent twips|--hanging-indent twips|--keep-next true|false|--keep-lines true|false|--occurrence n]"
                .to_owned(),
        )),
        }
    };

    match result {
        Ok(value) => {
            println!("{value}");
            if value.get("ok") == Some(&serde_json::Value::Bool(false)) {
                std::process::exit(1);
            }
        }
        Err((code, message)) => {
            println!(
                "{}",
                serde_json::json!({ "ok": false, "error": { "code": code, "message": message } })
            );
            std::process::exit(1);
        }
    }
}

fn set_table_cell_text(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<serde_json::Value, (&'static str, String)> {
    let (
        Some(input),
        Some(output),
        Some(row_label),
        Some(column_header),
        Some(expected),
        Some(replacement),
        occurrence,
        None,
    ) = (
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
    )
    else {
        return Err(("INVALID_ARGUMENTS", "usage: opensuite set-table-cell-text <input.docx> <output.docx> <row-label> <column-header> <expected-current-text> <replacement> [occurrence]".to_owned()));
    };
    let text = |value: std::ffi::OsString, name| {
        value
            .into_string()
            .map_err(|_| ("INVALID_ARGUMENTS", format!("{name} must be valid UTF-8")))
    };
    let row_label = text(row_label, "row label")?;
    let column_header = text(column_header, "column header")?;
    let expected_current_text = text(expected, "expected current text")?;
    let replacement = text(replacement, "replacement")?;
    let occurrence = occurrence
        .map(|value| {
            value
                .into_string()
                .ok()
                .and_then(|value| value.parse().ok())
                .ok_or_else(|| {
                    (
                        "INVALID_ARGUMENTS",
                        "occurrence must be a non-negative integer".to_owned(),
                    )
                })
        })
        .transpose()?;
    let package =
        opensuite_opc::Package::open(&input).map_err(|error| (error.code(), error.to_string()))?;
    let (main, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    Ok(opensuite_docx::set_table_cell_text(
        &package,
        &main,
        &source,
        &opensuite_protocol::SetTableCellText {
            target: opensuite_protocol::TableCellTarget {
                row_label,
                column_header,
                occurrence,
                handle: None,
            },
            expected_current_text,
            replacement,
            base_revision: None,
        },
        output,
    )
    .to_json())
}

fn set_content_control_text(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<serde_json::Value, (&'static str, String)> {
    let (Some(input), Some(output), Some(tag), Some(expected), Some(replacement), occurrence, None) = (
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
    ) else {
        return Err(("INVALID_ARGUMENTS", "usage: opensuite set-content-control-text <input.docx> <output.docx> <tag> <expected-current-text> <replacement> [occurrence]".to_owned()));
    };
    let text = |value: std::ffi::OsString, name| {
        value
            .into_string()
            .map_err(|_| ("INVALID_ARGUMENTS", format!("{name} must be valid UTF-8")))
    };
    let tag = text(tag, "tag")?;
    let expected_current_text = text(expected, "expected current text")?;
    let replacement = text(replacement, "replacement")?;
    let occurrence = occurrence
        .map(|value| {
            value
                .into_string()
                .ok()
                .and_then(|value| value.parse().ok())
                .ok_or_else(|| {
                    (
                        "INVALID_ARGUMENTS",
                        "occurrence must be a non-negative integer".to_owned(),
                    )
                })
        })
        .transpose()?;
    let package =
        opensuite_opc::Package::open(&input).map_err(|error| (error.code(), error.to_string()))?;
    let (main, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    Ok(opensuite_docx::set_content_control_text(
        &package,
        &main,
        &source,
        &opensuite_protocol::SetContentControlText {
            target: opensuite_protocol::ContentControlTarget {
                tag: Some(tag),
                alias: None,
                occurrence,
            },
            expected_current_text,
            replacement,
            base_revision: None,
        },
        output,
    )
    .to_json())
}

fn set_paragraph_formatting(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<serde_json::Value, (&'static str, String)> {
    use opensuite_protocol::{
        LineSpacing, LineSpacingRule, ParagraphAlignment, ParagraphFormattingPatch, PropertyPatch,
    };
    let (Some(input), Some(output), Some(target)) =
        (arguments.next(), arguments.next(), arguments.next())
    else {
        return Err((
            "INVALID_ARGUMENTS",
            "set-paragraph-formatting requires input, output, and target text".to_owned(),
        ));
    };
    let target = target.into_string().map_err(|_| {
        (
            "INVALID_ARGUMENTS",
            "target text must be valid UTF-8".to_owned(),
        )
    })?;
    let values = arguments
        .map(|value| {
            value.into_string().map_err(|_| {
                (
                    "INVALID_ARGUMENTS",
                    "formatting arguments must be valid UTF-8".to_owned(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut patch = ParagraphFormattingPatch::default();
    let mut occurrence = None;
    let mut line_rule = None;
    let mut index = 0;
    while index < values.len() {
        let flag = &values[index];
        let value = |index: &mut usize| -> Result<&str, (&'static str, String)> {
            *index += 1;
            values
                .get(*index)
                .map(String::as_str)
                .ok_or_else(|| ("INVALID_ARGUMENTS", format!("{flag} requires a value")))
        };
        let integer = |value: &str| {
            value
                .parse::<i32>()
                .map_err(|_| ("INVALID_ARGUMENTS", format!("{flag} must be an integer")))
        };
        match flag.as_str() {
            "--alignment" => {
                patch.alignment = Some(PropertyPatch::Set(match value(&mut index)? {
                    "left" => ParagraphAlignment::Left,
                    "center" => ParagraphAlignment::Center,
                    "right" => ParagraphAlignment::Right,
                    "both" => ParagraphAlignment::Both,
                    "distribute" => ParagraphAlignment::Distribute,
                    _ => {
                        return Err((
                            "INVALID_ARGUMENTS",
                            "alignment must be left, center, right, both, or distribute".to_owned(),
                        ));
                    }
                }))
            }
            "--clear-alignment" => patch.alignment = Some(PropertyPatch::Clear),
            "--space-before" => {
                patch.spacing_before_twips = Some(PropertyPatch::Set(integer(value(&mut index)?)?))
            }
            "--clear-space-before" => patch.spacing_before_twips = Some(PropertyPatch::Clear),
            "--space-after" => {
                patch.spacing_after_twips = Some(PropertyPatch::Set(integer(value(&mut index)?)?))
            }
            "--clear-space-after" => patch.spacing_after_twips = Some(PropertyPatch::Clear),
            "--line" => {
                patch.line_spacing = Some(PropertyPatch::Set(LineSpacing {
                    value: value(&mut index)?.parse().map_err(|_| {
                        (
                            "INVALID_ARGUMENTS",
                            "line must be a non-negative integer".to_owned(),
                        )
                    })?,
                    rule: None,
                }))
            }
            "--line-rule" => {
                line_rule = Some(match value(&mut index)? {
                    "auto" => LineSpacingRule::Auto,
                    "exact" => LineSpacingRule::Exact,
                    "atLeast" => LineSpacingRule::AtLeast,
                    _ => {
                        return Err((
                            "INVALID_ARGUMENTS",
                            "line rule must be auto, exact, or atLeast".to_owned(),
                        ));
                    }
                })
            }
            "--clear-line" => patch.line_spacing = Some(PropertyPatch::Clear),
            "--left-indent" => {
                patch.left_indent_twips = Some(PropertyPatch::Set(integer(value(&mut index)?)?))
            }
            "--clear-left-indent" => patch.left_indent_twips = Some(PropertyPatch::Clear),
            "--right-indent" => {
                patch.right_indent_twips = Some(PropertyPatch::Set(integer(value(&mut index)?)?))
            }
            "--clear-right-indent" => patch.right_indent_twips = Some(PropertyPatch::Clear),
            "--first-line-indent" => {
                patch.first_line_indent_twips =
                    Some(PropertyPatch::Set(integer(value(&mut index)?)?))
            }
            "--clear-first-line-indent" => {
                patch.first_line_indent_twips = Some(PropertyPatch::Clear)
            }
            "--hanging-indent" => {
                patch.hanging_indent_twips = Some(PropertyPatch::Set(integer(value(&mut index)?)?))
            }
            "--clear-hanging-indent" => patch.hanging_indent_twips = Some(PropertyPatch::Clear),
            "--keep-next" => {
                patch.keep_with_next = Some(PropertyPatch::Set(
                    value(&mut index)?.parse().map_err(|_| {
                        (
                            "INVALID_ARGUMENTS",
                            "keep-next must be true or false".to_owned(),
                        )
                    })?,
                ))
            }
            "--keep-lines" => {
                patch.keep_lines = Some(PropertyPatch::Set(value(&mut index)?.parse().map_err(
                    |_| {
                        (
                            "INVALID_ARGUMENTS",
                            "keep-lines must be true or false".to_owned(),
                        )
                    },
                )?))
            }
            "--clear-keep-next" => patch.keep_with_next = Some(PropertyPatch::Clear),
            "--clear-keep-lines" => patch.keep_lines = Some(PropertyPatch::Clear),
            "--occurrence" => {
                occurrence = Some(value(&mut index)?.parse().map_err(|_| {
                    (
                        "INVALID_ARGUMENTS",
                        "occurrence must be a non-negative integer".to_owned(),
                    )
                })?)
            }
            _ => {
                return Err((
                    "INVALID_ARGUMENTS",
                    format!("unknown formatting option {flag}"),
                ));
            }
        }
        index += 1;
    }
    if let Some(rule) = line_rule {
        let Some(PropertyPatch::Set(line)) = &mut patch.line_spacing else {
            return Err((
                "INVALID_ARGUMENTS",
                "--line-rule requires --line".to_owned(),
            ));
        };
        line.rule = Some(rule);
    }
    let package =
        opensuite_opc::Package::open(&input).map_err(|error| (error.code(), error.to_string()))?;
    let (main, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    Ok(opensuite_docx::set_paragraph_formatting(
        &package,
        &main,
        &source,
        &opensuite_protocol::SetParagraphFormatting {
            target: opensuite_protocol::TextTarget {
                text: target,
                occurrence,
            },
            formatting: patch,
            base_revision: None,
        },
        output,
    )
    .to_json())
}

fn set_text_formatting(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<serde_json::Value, (&'static str, String)> {
    use opensuite_protocol::{PropertyPatch, TextFormattingPatch};
    let (Some(input), Some(output), Some(target)) =
        (arguments.next(), arguments.next(), arguments.next())
    else {
        return Err((
            "INVALID_ARGUMENTS",
            "set-text-formatting requires input, output, and target text".to_owned(),
        ));
    };
    let target = target.into_string().map_err(|_| {
        (
            "INVALID_ARGUMENTS",
            "target text must be valid UTF-8".to_owned(),
        )
    })?;
    let values = arguments
        .map(|value| {
            value.into_string().map_err(|_| {
                (
                    "INVALID_ARGUMENTS",
                    "formatting arguments must be valid UTF-8".to_owned(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut formatting = TextFormattingPatch::default();
    let mut occurrence = None;
    let mut index = 0;
    while index < values.len() {
        let flag = &values[index];
        let value = |index: &mut usize| -> Result<&str, (&'static str, String)> {
            *index += 1;
            values
                .get(*index)
                .map(String::as_str)
                .ok_or_else(|| ("INVALID_ARGUMENTS", format!("{flag} requires a value")))
        };
        match flag.as_str() {
            "--bold" => {
                formatting.bold =
                    Some(PropertyPatch::Set(value(&mut index)?.parse().map_err(
                        |_| ("INVALID_ARGUMENTS", "bold must be true or false".to_owned()),
                    )?))
            }
            "--clear-bold" => formatting.bold = Some(PropertyPatch::Clear),
            "--italic" => {
                formatting.italic = Some(PropertyPatch::Set(value(&mut index)?.parse().map_err(
                    |_| {
                        (
                            "INVALID_ARGUMENTS",
                            "italic must be true or false".to_owned(),
                        )
                    },
                )?))
            }
            "--clear-italic" => formatting.italic = Some(PropertyPatch::Clear),
            "--font-size" => {
                formatting.font_size_half_points = Some(PropertyPatch::Set(
                    value(&mut index)?.parse().map_err(|_| {
                        (
                            "INVALID_ARGUMENTS",
                            "font size must be a positive half-point value".to_owned(),
                        )
                    })?,
                ))
            }
            "--clear-font-size" => formatting.font_size_half_points = Some(PropertyPatch::Clear),
            "--font-family" => {
                formatting.font_family = Some(PropertyPatch::Set(value(&mut index)?.to_owned()))
            }
            "--clear-font-family" => formatting.font_family = Some(PropertyPatch::Clear),
            "--occurrence" => {
                occurrence = Some(value(&mut index)?.parse().map_err(|_| {
                    (
                        "INVALID_ARGUMENTS",
                        "occurrence must be a non-negative integer".to_owned(),
                    )
                })?)
            }
            _ => {
                return Err((
                    "INVALID_ARGUMENTS",
                    format!("unknown text-formatting option {flag}"),
                ));
            }
        }
        index += 1;
    }
    let package =
        opensuite_opc::Package::open(&input).map_err(|error| (error.code(), error.to_string()))?;
    let (main, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    Ok(opensuite_docx::set_text_formatting(
        &package,
        &main,
        &source,
        &opensuite_protocol::SetTextFormatting {
            target: opensuite_protocol::TextTarget {
                text: target,
                occurrence,
            },
            formatting,
            base_revision: None,
        },
        output,
    )
    .to_json())
}

fn set_paragraph_style(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<serde_json::Value, (&'static str, String)> {
    let (Some(input), Some(output), Some(target)) =
        (arguments.next(), arguments.next(), arguments.next())
    else {
        return Err((
            "INVALID_ARGUMENTS",
            "set-paragraph-style requires input, output, and target text".to_owned(),
        ));
    };
    let target = target.into_string().map_err(|_| {
        (
            "INVALID_ARGUMENTS",
            "target text must be valid UTF-8".to_owned(),
        )
    })?;
    let values = arguments
        .map(|value| {
            value.into_string().map_err(|_| {
                (
                    "INVALID_ARGUMENTS",
                    "style arguments must be valid UTF-8".to_owned(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut style = None;
    let mut occurrence = None;
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--style" => {
                index += 1;
                style = Some(opensuite_protocol::PropertyPatch::Set(
                    values.get(index).cloned().ok_or_else(|| {
                        ("INVALID_ARGUMENTS", "--style requires a value".to_owned())
                    })?,
                ));
            }
            "--clear-style" => style = Some(opensuite_protocol::PropertyPatch::Clear),
            "--occurrence" => {
                index += 1;
                occurrence = Some(
                    values
                        .get(index)
                        .ok_or_else(|| {
                            (
                                "INVALID_ARGUMENTS",
                                "--occurrence requires a value".to_owned(),
                            )
                        })?
                        .parse()
                        .map_err(|_| {
                            (
                                "INVALID_ARGUMENTS",
                                "occurrence must be a non-negative integer".to_owned(),
                            )
                        })?,
                );
            }
            value => {
                return Err((
                    "INVALID_ARGUMENTS",
                    format!("unknown paragraph-style option {value}"),
                ));
            }
        }
        index += 1;
    }
    let Some(style) = style else {
        return Err((
            "INVALID_ARGUMENTS",
            "specify --style or --clear-style".to_owned(),
        ));
    };
    let package =
        opensuite_opc::Package::open(&input).map_err(|error| (error.code(), error.to_string()))?;
    let (main, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    Ok(opensuite_docx::set_paragraph_style(
        &package,
        &main,
        &source,
        &opensuite_protocol::SetParagraphStyle {
            target: opensuite_protocol::TextTarget {
                text: target,
                occurrence,
            },
            style,
            base_revision: None,
        },
        output,
    )
    .to_json())
}

fn replace_picture(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<serde_json::Value, (&'static str, String)> {
    let (Some(input), Some(output)) = (arguments.next(), arguments.next()) else {
        return Err((
            "INVALID_ARGUMENTS",
            "replace-picture requires input and output".to_owned(),
        ));
    };
    let values = arguments
        .map(|value| {
            value.into_string().map_err(|_| {
                (
                    "INVALID_ARGUMENTS",
                    "picture arguments must be UTF-8".to_owned(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut name = None;
    let mut description = None;
    let mut image = None;
    let mut occurrence = None;
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--name" => {
                index += 1;
                name = values.get(index).cloned();
            }
            "--description" => {
                index += 1;
                description = values.get(index).cloned();
            }
            "--image" => {
                index += 1;
                image = values.get(index).cloned();
            }
            "--occurrence" => {
                index += 1;
                occurrence = Some(
                    values
                        .get(index)
                        .ok_or_else(|| {
                            (
                                "INVALID_ARGUMENTS",
                                "--occurrence requires a value".to_owned(),
                            )
                        })?
                        .parse()
                        .map_err(|_| {
                            (
                                "INVALID_ARGUMENTS",
                                "occurrence must be an integer".to_owned(),
                            )
                        })?,
                );
            }
            other => {
                return Err((
                    "INVALID_ARGUMENTS",
                    format!("unknown picture option {other}"),
                ));
            }
        };
        index += 1;
    }
    let image = image.ok_or_else(|| ("INVALID_ARGUMENTS", "--image is required".to_owned()))?;
    let bytes = std::fs::read(&image).map_err(|error| ("INVALID_ARGUMENTS", error.to_string()))?;
    let content_type = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else {
        return Err((
            "INVALID_ARGUMENTS",
            "replacement must be PNG or JPEG".to_owned(),
        ));
    };
    let package =
        opensuite_opc::Package::open(&input).map_err(|error| (error.code(), error.to_string()))?;
    let (main, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    Ok(opensuite_docx::replace_picture(
        &package,
        &main,
        &source,
        &opensuite_protocol::ReplacePicture {
            target: opensuite_protocol::PictureTarget {
                name,
                description,
                occurrence,
            },
            replacement: opensuite_protocol::ImagePayload {
                content_type: content_type.to_owned(),
                bytes,
            },
            base_revision: None,
        },
        output,
    )
    .to_json())
}

fn delete_paragraph(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<serde_json::Value, (&'static str, String)> {
    let (Some(input), Some(output), Some(target), occurrence, None) = (
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
    ) else {
        return Err(("INVALID_ARGUMENTS", "usage: opensuite delete-paragraph <input.docx> <output.docx> <target-text> [occurrence]".to_owned()));
    };
    let target = target.into_string().map_err(|_| {
        (
            "INVALID_ARGUMENTS",
            "target text must be valid UTF-8".to_owned(),
        )
    })?;
    let occurrence = occurrence
        .map(|value| {
            value
                .into_string()
                .ok()
                .and_then(|value| value.parse().ok())
                .ok_or_else(|| {
                    (
                        "INVALID_ARGUMENTS",
                        "occurrence must be a non-negative integer".to_owned(),
                    )
                })
        })
        .transpose()?;
    let package =
        opensuite_opc::Package::open(&input).map_err(|error| (error.code(), error.to_string()))?;
    let (main, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    Ok(opensuite_docx::delete_paragraph(
        &package,
        &main,
        &source,
        &opensuite_protocol::DeleteParagraph {
            target: opensuite_protocol::TextTarget {
                text: target,
                occurrence,
            },
            base_revision: None,
        },
        output,
    )
    .to_json())
}

fn insert_paragraph_after(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<serde_json::Value, (&'static str, String)> {
    let (Some(input), Some(output), Some(anchor), Some(text), occurrence, None) = (
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
    ) else {
        return Err(("INVALID_ARGUMENTS", "usage: opensuite insert-paragraph-after <input.docx> <output.docx> <anchor-text> <new-paragraph-text> [occurrence]".to_owned()));
    };
    let anchor = anchor.into_string().map_err(|_| {
        (
            "INVALID_ARGUMENTS",
            "anchor text must be valid UTF-8".to_owned(),
        )
    })?;
    let text = text.into_string().map_err(|_| {
        (
            "INVALID_ARGUMENTS",
            "new paragraph text must be valid UTF-8".to_owned(),
        )
    })?;
    let occurrence = occurrence
        .map(|value| {
            value
                .into_string()
                .ok()
                .and_then(|value| value.parse().ok())
                .ok_or_else(|| {
                    (
                        "INVALID_ARGUMENTS",
                        "occurrence must be a non-negative integer".to_owned(),
                    )
                })
        })
        .transpose()?;
    let package =
        opensuite_opc::Package::open(&input).map_err(|error| (error.code(), error.to_string()))?;
    let (main, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    Ok(opensuite_docx::insert_paragraph_after(
        &package,
        &main,
        &source,
        &opensuite_protocol::InsertParagraphAfter {
            anchor: opensuite_protocol::TextTarget {
                text: anchor,
                occurrence,
            },
            text,
            base_revision: None,
        },
        output,
    )
    .to_json())
}

fn find_text(
    path: std::ffi::OsString,
    text: std::ffi::OsString,
) -> Result<serde_json::Value, (&'static str, String)> {
    let text = text.into_string().map_err(|_| {
        (
            "INVALID_ARGUMENTS",
            "text query must be valid UTF-8".to_owned(),
        )
    })?;
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (_, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    opensuite_docx::find_text(&source, &opensuite_protocol::FindText { text })
        .map(|result| result.to_json())
        .map_err(|error| (error.code(), error.to_string()))
}

fn inspect_context(
    mut arguments: impl Iterator<Item = std::ffi::OsString>,
) -> Result<serde_json::Value, (&'static str, String)> {
    let (Some(path), Some(text), occurrence, before, after, None) = (
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
    ) else {
        return Err((
            "INVALID_ARGUMENTS",
            "usage: opensuite inspect-context <input.docx> <text> [occurrence] [before] [after]"
                .to_owned(),
        ));
    };
    let text = text.into_string().map_err(|_| {
        (
            "INVALID_ARGUMENTS",
            "text target must be valid UTF-8".to_owned(),
        )
    })?;
    let parse_count = |value: std::ffi::OsString, name: &str| {
        value
            .into_string()
            .ok()
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| {
                (
                    "INVALID_ARGUMENTS",
                    format!("{name} must be a non-negative integer"),
                )
            })
    };
    let occurrence = occurrence
        .map(|value| parse_count(value, "occurrence"))
        .transpose()?;
    let before = before
        .map(|value| parse_count(value, "before"))
        .transpose()?
        .unwrap_or(1);
    let after = after
        .map(|value| parse_count(value, "after"))
        .transpose()?
        .unwrap_or(1);
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (_, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    opensuite_docx::inspect_text_context(
        &source,
        &opensuite_protocol::InspectTextContext {
            target: opensuite_protocol::TextTarget { text, occurrence },
            before,
            after,
        },
    )
    .map(|result| result.to_json())
    .map_err(|error| (error.code(), error.to_string()))
}

fn replace_text(
    input: std::ffi::OsString,
    output: std::ffi::OsString,
    target: std::ffi::OsString,
    replacement: std::ffi::OsString,
) -> Result<serde_json::Value, (&'static str, String)> {
    let target = target.into_string().map_err(|_| {
        (
            "INVALID_ARGUMENTS",
            "text target must be valid UTF-8".to_owned(),
        )
    })?;
    let replacement = replacement.into_string().map_err(|_| {
        (
            "INVALID_ARGUMENTS",
            "replacement text must be valid UTF-8".to_owned(),
        )
    })?;
    let package =
        opensuite_opc::Package::open(&input).map_err(|error| (error.code(), error.to_string()))?;
    let (main, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let operation = opensuite_protocol::ReplaceText {
        target: opensuite_protocol::TextTarget {
            text: target.clone(),
            occurrence: None,
        },
        expected_current_text: target,
        replacement,
        base_revision: None,
    };
    Ok(opensuite_docx::replace_text(&package, &main, &source, &operation, output).to_json())
}

fn inspect_comments(path: std::ffi::OsString) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (main, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let comments = document
        .comments(&package, &main)
        .map_err(|error| (error.code(), error.to_string()))?;
    let values = comments
        .comments()
        .map(|comment| {
            let metadata = comment.metadata();
            Ok(serde_json::json!({
                "id": comment.id(),
                "author": metadata.author,
                "initials": metadata.initials,
                "date": metadata.date,
                "text": comment.text().map_err(|error| (error.code(), error.to_string()))?,
                "has_range": comment.has_range(),
                "has_reference": comment.has_reference(),
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let errors = comments
        .errors()
        .map(|error| serde_json::json!({ "code": error.code() }))
        .collect::<Vec<_>>();
    Ok(
        serde_json::json!({ "ok": true, "comment_count": values.len(), "comments": values, "errors": errors }),
    )
}

fn inspect_tracked_changes(
    path: std::ffi::OsString,
) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (_, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let tracked_changes = document
        .tracked_changes()
        .map(|change| {
            let metadata = change.metadata();
            Ok(serde_json::json!({
                "kind": tracked_change_kind_name(change.kind()),
                "id": metadata.id,
                "author": metadata.author,
                "date": metadata.date,
                "text": change.text().map_err(|error| (error.code(), error.to_string()))?,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        serde_json::json!({ "ok": true, "tracked_change_count": tracked_changes.len(), "tracked_changes": tracked_changes }),
    )
}

fn tracked_change_kind_name(kind: opensuite_docx::TrackedChangeKind) -> &'static str {
    match kind {
        opensuite_docx::TrackedChangeKind::Insertion => "insertion",
        opensuite_docx::TrackedChangeKind::Deletion => "deletion",
        opensuite_docx::TrackedChangeKind::MoveFrom => "move_from",
        opensuite_docx::TrackedChangeKind::MoveTo => "move_to",
    }
}

fn inspect_revision_view(
    path: std::ffi::OsString,
    view_name: std::ffi::OsString,
) -> Result<serde_json::Value, (&'static str, String)> {
    let (view, view_name) = match view_name.to_str() {
        Some("current") => (opensuite_docx::RevisionView::Current, "current"),
        Some("original") => (opensuite_docx::RevisionView::Original, "original"),
        _ => {
            return Err((
                "INVALID_REVISION_VIEW",
                "revision view must be current or original".to_owned(),
            ));
        }
    };
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (part, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let blocks = document
        .blocks()
        .map(|block| inspect_block_for_view(block, view))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(serde_json::json!({
        "ok": true,
        "part_name": part.name.as_str(),
        "revision_view": view_name,
        "block_count": blocks.len(),
        "blocks": blocks,
    }))
}

fn inspect_block_for_view(
    block: opensuite_docx::BodyBlock<'_>,
    view: opensuite_docx::RevisionView,
) -> Result<serde_json::Value, (&'static str, String)> {
    match block {
        opensuite_docx::BodyBlock::Paragraph(paragraph) => Ok(serde_json::json!({
            "type": "paragraph",
            "text": paragraph.text_for_view(view).map_err(|error| (error.code(), error.to_string()))?,
            "run_count": paragraph.runs().count(),
        })),
        opensuite_docx::BodyBlock::Table(table) => {
            let rows = table
                .rows()
                .map(|row| {
                    let cells = row
                        .cells()
                        .map(|cell| {
                            cell.text_for_view(view)
                                .map(|text| serde_json::json!({ "text": text }))
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|error| (error.code(), error.to_string()))?;
                    Ok(serde_json::json!({ "cells": cells }))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(serde_json::json!({ "type": "table", "row_count": rows.len(), "rows": rows }))
        }
    }
}

fn capabilities() -> Result<serde_json::Value, (&'static str, String)> {
    let capabilities = opensuite_protocol::RuntimeCapabilities::docx(env!("CARGO_PKG_VERSION"));
    let mut manifest = capabilities.to_json();
    let Some(output) = manifest.as_object_mut() else {
        return Err((
            "PROTOCOL_SERIALIZATION_ERROR",
            "capability manifest is not an object".to_owned(),
        ));
    };
    output.insert("ok".to_owned(), serde_json::json!(true));
    Ok(manifest)
}

fn inspect_content_controls(
    path: std::ffi::OsString,
) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (_, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let content_controls = document
        .content_controls()
        .map(|control| {
            let properties = control.properties();
            Ok(serde_json::json!({
                "kind": content_control_kind_name(properties.kind),
                "id": properties.id,
                "alias": properties.alias,
                "tag": properties.tag,
                "text": control.visible_text().map_err(|error| (error.code(), error.to_string()))?,
                "lock": properties.lock,
                "placeholder": properties.placeholder,
                "data_binding": properties.data_binding.map(|binding| serde_json::json!({ "store_item_id": binding.store_item_id, "xpath": binding.xpath, "prefix_mappings": binding.prefix_mappings })),
                "items": properties.items.into_iter().map(|item| serde_json::json!({ "display_text": item.display_text, "value": item.value })).collect::<Vec<_>>(),
                "date": properties.date.map(|date| serde_json::json!({ "format": date.format, "language_id": date.language_id, "calendar": date.calendar })),
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        serde_json::json!({ "ok": true, "content_control_count": content_controls.len(), "content_controls": content_controls }),
    )
}

fn content_control_kind_name(kind: opensuite_docx::ContentControlKind) -> &'static str {
    match kind {
        opensuite_docx::ContentControlKind::Text => "text",
        opensuite_docx::ContentControlKind::RichText => "rich_text",
        opensuite_docx::ContentControlKind::Date => "date",
        opensuite_docx::ContentControlKind::DropDownList => "drop_down_list",
        opensuite_docx::ContentControlKind::ComboBox => "combo_box",
        opensuite_docx::ContentControlKind::CheckBox => "check_box",
        opensuite_docx::ContentControlKind::Picture => "picture",
        opensuite_docx::ContentControlKind::Group => "group",
        opensuite_docx::ContentControlKind::RepeatingSection => "repeating_section",
        opensuite_docx::ContentControlKind::RepeatingSectionItem => "repeating_section_item",
        opensuite_docx::ContentControlKind::Unknown => "unknown",
    }
}

fn inspect_fields(path: std::ffi::OsString) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (_, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let fields = document.fields();
    let values = fields
        .iter()
        .map(|field| {
            Ok(serde_json::json!({
                "kind": match field.kind() { opensuite_docx::FieldKind::Simple => "simple", opensuite_docx::FieldKind::Complex => "complex" },
                "instruction": field.instruction().map_err(|error| (error.code(), error.to_string()))?,
                "result_text": field.result_text().map_err(|error| (error.code(), error.to_string()))?,
                "has_separator": field.has_separator(),
                "state": match field.state() { opensuite_docx::FieldState::Complete => "complete", opensuite_docx::FieldState::Unterminated => "unterminated" },
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let errors = fields
        .errors()
        .map(|error| serde_json::json!({ "code": error.code() }))
        .collect::<Vec<_>>();
    Ok(
        serde_json::json!({ "ok": true, "field_count": values.len(), "fields": values, "errors": errors }),
    )
}

fn inspect_images(path: std::ffi::OsString) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let main = package
        .main_office_document()
        .map_err(|error| (error.code(), error.to_string()))?;
    let source = opensuite_docx::SourceDocument::parse(
        package
            .read_part(&main)
            .map_err(|error| (error.code(), error.to_string()))?,
    )
    .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let images = document
        .pictures()
        .map(|picture| {
            let extent = picture
                .extent()
                .map_err(|error| (error.code(), error.to_string()))?;
            let metadata = picture.metadata();
            let reference = picture
                .image_reference(&package, &main)
                .map_err(|error| (error.code(), error.to_string()))?;
            let mut value = serde_json::Map::new();
            value.insert(
                "kind".to_owned(),
                serde_json::json!(match picture.kind() {
                    opensuite_docx::PictureKind::Inline => "inline",
                    opensuite_docx::PictureKind::Anchored => "anchored",
                }),
            );
            value.insert(
                "width_emu".to_owned(),
                serde_json::json!(extent.map(|extent| extent.width_emu)),
            );
            value.insert(
                "height_emu".to_owned(),
                serde_json::json!(extent.map(|extent| extent.height_emu)),
            );
            if let Some(metadata) = metadata {
                value.insert("name".to_owned(), serde_json::json!(metadata.name));
                value.insert(
                    "description".to_owned(),
                    serde_json::json!(metadata.description),
                );
                value.insert("title".to_owned(), serde_json::json!(metadata.title));
            }
            match reference {
                opensuite_docx::ImageReference::Embedded(image) => {
                    value.insert(
                        "part_name".to_owned(),
                        serde_json::json!(image.part.name.as_str()),
                    );
                    value.insert(
                        "content_type".to_owned(),
                        serde_json::json!(image.part.content_type.as_str()),
                    );
                    value.insert("size_bytes".to_owned(), serde_json::json!(image.size_bytes));
                }
                opensuite_docx::ImageReference::LinkedExternal(target) => {
                    value.insert("linked_target".to_owned(), serde_json::json!(target));
                }
                opensuite_docx::ImageReference::LinkedInternal(part) => {
                    value.insert(
                        "linked_part_name".to_owned(),
                        serde_json::json!(part.name.as_str()),
                    );
                }
            }
            Ok(serde_json::Value::Object(value))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({ "ok": true, "image_count": images.len(), "images": images }))
}

fn inspect_references(
    path: std::ffi::OsString,
) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let main = package
        .main_office_document()
        .map_err(|error| (error.code(), error.to_string()))?;
    let source = opensuite_docx::SourceDocument::parse(
        package
            .read_part(&main)
            .map_err(|error| (error.code(), error.to_string()))?,
    )
    .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let hyperlinks = document
        .hyperlinks()
        .map(|hyperlink| {
            let target = hyperlink
                .target(&package, &main)
                .map_err(|error| (error.code(), error.to_string()))?;
            let (kind, target) = match target {
                Some(opensuite_docx::HyperlinkTarget::External(target)) => ("external", target),
                Some(opensuite_docx::HyperlinkTarget::InternalAnchor(target)) => ("internal", target),
                Some(opensuite_docx::HyperlinkTarget::InternalPart(part)) => {
                    ("internal_part", part.name.as_str().to_owned())
                }
                None => ("none", String::new()),
            };
            Ok(serde_json::json!({ "text": hyperlink.text().map_err(|error| (error.code(), error.to_string()))?, "target": target, "kind": kind }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let bookmarks = document
        .bookmarks()
        .map_err(|error| (error.code(), error.to_string()))?
        .into_iter()
        .map(|bookmark| serde_json::json!({ "name": bookmark.name, "id": bookmark.id.0.to_string() }))
        .collect::<Vec<_>>();
    Ok(serde_json::json!({ "ok": true, "hyperlinks": hyperlinks, "bookmarks": bookmarks }))
}

fn inspect_headers_footers(
    path: std::ffi::OsString,
) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let main = package
        .main_office_document()
        .map_err(|error| (error.code(), error.to_string()))?;
    let source = opensuite_docx::SourceDocument::parse(
        package
            .read_part(&main)
            .map_err(|error| (error.code(), error.to_string()))?,
    )
    .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let sections = document
        .sections()
        .map(|section| {
            Ok(serde_json::json!({
                "headers": section.header_references().map(|reference| inspect_header_footer(&package, &main, &reference)).collect::<Result<Vec<_>, _>>()?,
                "footers": section.footer_references().map(|reference| inspect_header_footer(&package, &main, &reference)).collect::<Result<Vec<_>, _>>()?,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({ "ok": true, "sections": sections }))
}

fn inspect_header_footer(
    package: &opensuite_opc::Package,
    main: &opensuite_opc::Part,
    reference: &opensuite_docx::HeaderFooterReference,
) -> Result<serde_json::Value, (&'static str, String)> {
    let content = match reference.kind() {
        opensuite_docx::HeaderFooterKind::Header => {
            opensuite_docx::load_header(package, main, reference)
        }
        opensuite_docx::HeaderFooterKind::Footer => {
            opensuite_docx::load_footer(package, main, reference)
        }
    }
    .map_err(|error| (error.code(), error.to_string()))?;
    let paragraphs = content
        .blocks()
        .filter_map(|block| match block {
            opensuite_docx::BodyBlock::Paragraph(paragraph) => Some(paragraph.text()),
            opensuite_docx::BodyBlock::Table(_) => None,
        })
        .map(|text| {
            text.map(|text| serde_json::json!({ "text": text }))
                .map_err(|error| (error.code(), error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        serde_json::json!({ "type": header_footer_type_name(reference.reference_type()), "part_name": content.part().name.as_str(), "paragraphs": paragraphs }),
    )
}

fn header_footer_type_name(value: &opensuite_docx::HeaderFooterType) -> String {
    match value {
        opensuite_docx::HeaderFooterType::Default => "default".to_owned(),
        opensuite_docx::HeaderFooterType::First => "first".to_owned(),
        opensuite_docx::HeaderFooterType::Even => "even".to_owned(),
        opensuite_docx::HeaderFooterType::Unknown(value) => value.clone(),
    }
}

fn inspect_sections(path: std::ffi::OsString) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (_, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let sections = document
        .sections()
        .map(|section| {
            let properties = section.properties();
            Ok(serde_json::json!({
                "type": properties.section_type().map(section_type_name),
                "page_size": properties.page_size().map_err(|error| (error.code(), error.to_string()))?.map(page_size_json),
                "margins": properties.page_margins().map_err(|error| (error.code(), error.to_string()))?.map(page_margins_json),
                "columns": properties.columns().map_err(|error| (error.code(), error.to_string()))?.map(columns_json),
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({ "ok": true, "section_count": sections.len(), "sections": sections }))
}

fn section_type_name(value: opensuite_docx::SectionType) -> String {
    match value {
        opensuite_docx::SectionType::NextPage => "nextPage".to_owned(),
        opensuite_docx::SectionType::Continuous => "continuous".to_owned(),
        opensuite_docx::SectionType::EvenPage => "evenPage".to_owned(),
        opensuite_docx::SectionType::OddPage => "oddPage".to_owned(),
        opensuite_docx::SectionType::Unknown(value) => value,
    }
}

fn page_size_json(value: opensuite_docx::PageSize) -> serde_json::Value {
    serde_json::json!({ "width_twips": value.width_twips, "height_twips": value.height_twips, "orientation": value.orientation.map(orientation_name) })
}
fn orientation_name(value: opensuite_docx::PageOrientation) -> String {
    match value {
        opensuite_docx::PageOrientation::Portrait => "portrait".to_owned(),
        opensuite_docx::PageOrientation::Landscape => "landscape".to_owned(),
        opensuite_docx::PageOrientation::Unknown(value) => value,
    }
}
fn page_margins_json(value: opensuite_docx::PageMargins) -> serde_json::Value {
    serde_json::json!({ "top_twips": value.top_twips, "bottom_twips": value.bottom_twips, "left_twips": value.left_twips, "right_twips": value.right_twips, "header_twips": value.header_twips, "footer_twips": value.footer_twips, "gutter_twips": value.gutter_twips })
}
fn columns_json(value: opensuite_docx::Columns) -> serde_json::Value {
    serde_json::json!({ "count": value.count, "spacing_twips": value.spacing_twips, "equal_width": value.equal_width })
}

fn inspect_numbering(
    path: std::ffi::OsString,
) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let main = package
        .main_office_document()
        .map_err(|error| (error.code(), error.to_string()))?;
    let Some(numbering) = opensuite_docx::load_numbering(&package, &main)
        .map_err(|error| (error.code(), error.to_string()))?
    else {
        return Ok(serde_json::json!({"ok":true,"abstract_count":0,"instance_count":0,"lists":[]}));
    };
    let mut instances: Vec<_> = numbering.instances().collect();
    instances.sort_by_key(|instance| instance.num_id.0);
    let lists = instances.into_iter().map(|instance| {
        let levels = (0_u8..=8).filter_map(|level| numbering.resolve(opensuite_docx::ListReference { num_id: instance.num_id, level }).ok()).map(|level| serde_json::json!({"level":level.level,"format":number_format_name(&level.format),"text":level.text,"start":level.start,"suffix":level.suffix})).collect::<Vec<_>>();
        serde_json::json!({"num_id":instance.num_id.0,"abstract_num_id":instance.abstract_num_id.0,"levels":levels})
    }).collect::<Vec<_>>();
    Ok(
        serde_json::json!({"ok":true,"abstract_count":numbering.abstract_count(),"instance_count":numbering.instance_count(),"lists":lists}),
    )
}

fn number_format_name(format: &opensuite_docx::NumberFormat) -> &str {
    match format {
        opensuite_docx::NumberFormat::Decimal => "decimal",
        opensuite_docx::NumberFormat::UpperRoman => "upperRoman",
        opensuite_docx::NumberFormat::LowerRoman => "lowerRoman",
        opensuite_docx::NumberFormat::UpperLetter => "upperLetter",
        opensuite_docx::NumberFormat::LowerLetter => "lowerLetter",
        opensuite_docx::NumberFormat::Bullet => "bullet",
        opensuite_docx::NumberFormat::None => "none",
        opensuite_docx::NumberFormat::Unknown(value) => value,
    }
}

fn inspect_paragraphs(
    path: std::ffi::OsString,
) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (part, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let styles = opensuite_docx::load_styles(&package, &part)
        .map_err(|error| (error.code(), error.to_string()))?;
    let paragraphs = document
        .paragraphs()
        .enumerate()
        .map(|(index, paragraph)| {
            let mut item = serde_json::Map::new();
            item.insert("index".to_owned(), serde_json::json!(index));
            item.insert(
                "text".to_owned(),
                serde_json::json!(
                    paragraph
                        .text()
                        .map_err(|error| (error.code(), error.to_string()))?
                ),
            );
            if let Some(style_id) = paragraph.style_id() {
                item.insert("style_id".to_owned(), serde_json::json!(style_id.as_str()));
            }
            if let Some(styles) = styles.as_ref() {
                item.insert(
                    "formatting".to_owned(),
                    paragraph_formatting_json(
                        paragraph
                            .effective_formatting(styles)
                            .map_err(|error| (error.code(), error.to_string()))?,
                    ),
                );
            }
            Ok(serde_json::Value::Object(item))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({ "ok": true, "paragraphs": paragraphs }))
}

fn paragraph_formatting_json(formatting: opensuite_docx::ParagraphFormatting) -> serde_json::Value {
    let mut fields = serde_json::Map::new();
    if let Some(alignment) = formatting.alignment {
        fields.insert(
            "alignment".to_owned(),
            serde_json::json!(match alignment {
                opensuite_docx::ParagraphAlignment::Left => "left",
                opensuite_docx::ParagraphAlignment::Center => "center",
                opensuite_docx::ParagraphAlignment::Right => "right",
                opensuite_docx::ParagraphAlignment::Both => "both",
                opensuite_docx::ParagraphAlignment::Distribute => "distribute",
            }),
        );
    }
    if let Some(value) = formatting.spacing_before_twips {
        fields.insert("spacing_before_twips".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = formatting.spacing_after_twips {
        fields.insert("spacing_after_twips".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = formatting.line_spacing {
        let rule = match value.rule {
            Some(opensuite_docx::LineSpacingRule::Auto) => "auto",
            Some(opensuite_docx::LineSpacingRule::Exact) => "exact",
            Some(opensuite_docx::LineSpacingRule::AtLeast) => "atLeast",
            None => "unspecified",
        };
        fields.insert(
            "line_spacing".to_owned(),
            serde_json::json!({ "value": value.value, "rule": rule }),
        );
    }
    if let Some(value) = formatting.left_indent_twips {
        fields.insert("left_indent_twips".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = formatting.right_indent_twips {
        fields.insert("right_indent_twips".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = formatting.first_line_indent_twips {
        fields.insert(
            "first_line_indent_twips".to_owned(),
            serde_json::json!(value),
        );
    }
    if let Some(value) = formatting.hanging_indent_twips {
        fields.insert("hanging_indent_twips".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = formatting.keep_with_next {
        fields.insert("keep_with_next".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = formatting.keep_lines {
        fields.insert("keep_lines".to_owned(), serde_json::json!(value));
    }
    serde_json::Value::Object(fields)
}

fn inspect_styles(path: std::ffi::OsString) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let main = package
        .main_office_document()
        .map_err(|error| (error.code(), error.to_string()))?;
    let Some(styles) = opensuite_docx::load_styles(&package, &main)
        .map_err(|error| (error.code(), error.to_string()))?
    else {
        return Ok(serde_json::json!({ "ok": true, "style_count": 0, "styles": [] }));
    };
    let mut styles: Vec<_> = styles.styles().collect();
    styles.sort_by_key(|style| style.id().as_str());
    let styles = styles
        .into_iter()
        .map(|style| serde_json::json!({
            "id": style.id().as_str(),
            "name": style.name(),
            "type": match style.style_type() { opensuite_docx::StyleType::Paragraph => "paragraph", opensuite_docx::StyleType::Character => "character" },
            "based_on": style.based_on().map(opensuite_docx::StyleId::as_str),
        }))
        .collect::<Vec<_>>();
    Ok(serde_json::json!({ "ok": true, "style_count": styles.len(), "styles": styles }))
}

fn inspect_docx(path: std::ffi::OsString) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (part, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let blocks = document
        .blocks()
        .map(|block| inspect_block(block))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(serde_json::json!({
        "ok": true,
        "part_name": part.name.as_str(),
        "block_count": blocks.len(),
        "blocks": blocks,
    }))
}

fn inspect_block(
    block: opensuite_docx::BodyBlock<'_>,
) -> Result<serde_json::Value, (&'static str, String)> {
    match block {
        opensuite_docx::BodyBlock::Paragraph(paragraph) => Ok(serde_json::json!({
            "type": "paragraph",
            "text": paragraph.text().map_err(|error| (error.code(), error.to_string()))?,
            "run_count": paragraph.runs().count(),
        })),
        opensuite_docx::BodyBlock::Table(table) => {
            let rows = table
                .rows()
                .map(|row| {
                    let cells = row
                        .cells()
                        .map(|cell| cell.text().map(|text| serde_json::json!({ "text": text })))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|error| (error.code(), error.to_string()))?;
                    Ok(serde_json::json!({ "cells": cells }))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(serde_json::json!({ "type": "table", "row_count": rows.len(), "rows": rows }))
        }
    }
}

fn inspect_source(path: std::ffi::OsString) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (part, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let root = source.node(source.root()).ok_or((
        "SOURCE_SPAN_INCONSISTENCY",
        "source root is missing".to_owned(),
    ))?;
    let opensuite_docx::SourceNodeKind::Element { name, .. } = root.kind() else {
        return Err((
            "SOURCE_SPAN_INCONSISTENCY",
            "source root is not an element".to_owned(),
        ));
    };

    Ok(serde_json::json!({
        "ok": true,
        "part_name": part.name.as_str(),
        "source_bytes": source.original_bytes().len(),
        "node_count": source.node_count(),
        "element_count": source.element_count(),
        "text_count": source.text_count(),
        "root": {
            "local_name": name.local_name(),
            "namespace_uri": name.namespace_uri(),
        },
    }))
}

fn inspect(path: std::ffi::OsString) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let main_document = package
        .main_office_document()
        .map_err(|error| (error.code(), error.to_string()))?;

    Ok(serde_json::json!({
        "ok": true,
        "entry_count": package.entry_count(),
        "part_count": package.part_count(),
        "main_document": {
            "part_name": main_document.name.as_str(),
            "content_type": main_document.content_type.as_str(),
        },
    }))
}
