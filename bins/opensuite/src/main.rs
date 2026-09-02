fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let result = match (arguments.next(), arguments.next(), arguments.next()) {
        (Some(command), Some(path), None) if command == "inspect" => inspect(path),
        (Some(command), Some(path), None) if command == "inspect-source" => inspect_source(path),
        (Some(command), Some(path), None) if command == "inspect-docx" => inspect_docx(path),
        _ => Err((
            "INVALID_ARGUMENTS",
            "usage: opensuite <inspect|inspect-source|inspect-docx> <path-to-office-file>"
                .to_owned(),
        )),
    };

    match result {
        Ok(value) => println!("{value}"),
        Err((code, message)) => {
            println!(
                "{}",
                serde_json::json!({ "ok": false, "error": { "code": code, "message": message } })
            );
            std::process::exit(1);
        }
    }
}

fn inspect_docx(path: std::ffi::OsString) -> Result<serde_json::Value, (&'static str, String)> {
    let package =
        opensuite_opc::Package::open(&path).map_err(|error| (error.code(), error.to_string()))?;
    let (part, source) = opensuite_docx::open_main_source(&package)
        .map_err(|error| (error.code(), error.to_string()))?;
    let document = opensuite_docx::DocxDocument::new(&source)
        .map_err(|error| (error.code(), error.to_string()))?;
    let paragraphs = document
        .paragraphs()
        .map(|paragraph| {
            let text = paragraph
                .text()
                .map_err(|error| (error.code(), error.to_string()))?;
            Ok(serde_json::json!({ "text": text, "run_count": paragraph.runs().count() }))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(serde_json::json!({
        "ok": true,
        "part_name": part.name.as_str(),
        "paragraph_count": paragraphs.len(),
        "paragraphs": paragraphs,
    }))
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
