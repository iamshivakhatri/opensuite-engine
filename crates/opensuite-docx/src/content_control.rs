use crate::{NodeId, SemanticError, SourceDocument, SourceNodeKind};

const WORD: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];
const WORD_2010: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
const WORD_2012: &str = "http://schemas.microsoft.com/office/word/2012/wordml";

/// A read-only WordprocessingML content control backed by source XML.
pub struct ContentControl<'a> {
    source: &'a SourceDocument,
    source_id: NodeId,
    properties_id: Option<NodeId>,
    content_id: Option<NodeId>,
}

impl ContentControl<'_> {
    pub fn source_id(&self) -> NodeId {
        self.source_id
    }

    pub fn properties_id(&self) -> Option<NodeId> {
        self.properties_id
    }

    pub fn content_id(&self) -> Option<NodeId> {
        self.content_id
    }

    pub fn properties(&self) -> ContentControlProperties {
        let Some(properties_id) = self.properties_id else {
            return ContentControlProperties::default();
        };
        let mut properties = ContentControlProperties {
            source_id: Some(properties_id),
            ..Default::default()
        };
        for id in self.source.children(properties_id) {
            let value = self.source.node(id).and_then(|node| node.attribute("val"));
            if word(self.source, id, "id") {
                properties.id = value.and_then(|value| value.parse().ok());
            } else if word(self.source, id, "alias") {
                properties.alias = value.map(str::to_owned);
            } else if word(self.source, id, "tag") {
                properties.tag = value.map(str::to_owned);
            } else if word(self.source, id, "lock") {
                properties.lock = value.map(str::to_owned);
            } else if word(self.source, id, "placeholder") {
                properties.placeholder = child(self.source, id, "docPart")
                    .and_then(|id| self.source.node(id))
                    .and_then(|node| node.attribute("val"))
                    .map(str::to_owned);
            } else if word(self.source, id, "dataBinding") {
                properties.data_binding = Some(DataBinding {
                    store_item_id: self
                        .source
                        .node(id)
                        .and_then(|node| node.attribute("storeItemID"))
                        .map(str::to_owned),
                    xpath: self
                        .source
                        .node(id)
                        .and_then(|node| node.attribute("xpath"))
                        .map(str::to_owned),
                    prefix_mappings: self
                        .source
                        .node(id)
                        .and_then(|node| node.attribute("prefixMappings"))
                        .map(str::to_owned),
                });
            } else if word(self.source, id, "date") {
                properties.date = Some(DateMetadata {
                    format: child(self.source, id, "dateFormat")
                        .and_then(|id| self.source.node(id))
                        .and_then(|node| node.attribute("val"))
                        .map(str::to_owned),
                    language_id: child(self.source, id, "lid")
                        .and_then(|id| self.source.node(id))
                        .and_then(|node| node.attribute("val"))
                        .map(str::to_owned),
                    calendar: child(self.source, id, "calendar")
                        .and_then(|id| self.source.node(id))
                        .and_then(|node| node.attribute("val"))
                        .map(str::to_owned),
                });
            } else if word(self.source, id, "dropDownList") || word(self.source, id, "comboBox") {
                properties.items = self
                    .source
                    .children(id)
                    .filter(|item| word(self.source, *item, "listItem"))
                    .map(|item| ContentControlListItem {
                        display_text: self
                            .source
                            .node(item)
                            .and_then(|node| node.attribute("displayText"))
                            .map(str::to_owned),
                        value: self
                            .source
                            .node(item)
                            .and_then(|node| node.attribute("value"))
                            .map(str::to_owned),
                    })
                    .collect();
            }
        }
        properties.kind = kind(self.source, properties_id);
        properties
    }

    pub fn visible_text(&self) -> Result<String, SemanticError> {
        let Some(content_id) = self.content_id else {
            return Ok(String::new());
        };
        let span = self
            .source
            .node(content_id)
            .ok_or(SemanticError::InvalidTextNode)?
            .span();
        self.source
            .node_ids()
            .filter(|id| word(self.source, *id, "t"))
            .filter(|id| {
                self.source.node(*id).is_some_and(|node| {
                    node.span().start > span.start && node.span().end < span.end
                })
            })
            .try_fold(String::new(), |mut text, id| {
                text.push_str(&crate::semantic::text_value(self.source, id)?);
                Ok(text)
            })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContentControlProperties {
    pub source_id: Option<NodeId>,
    pub id: Option<i64>,
    pub alias: Option<String>,
    pub tag: Option<String>,
    pub kind: ContentControlKind,
    pub lock: Option<String>,
    pub placeholder: Option<String>,
    pub data_binding: Option<DataBinding>,
    pub items: Vec<ContentControlListItem>,
    pub date: Option<DateMetadata>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DataBinding {
    pub store_item_id: Option<String>,
    pub xpath: Option<String>,
    pub prefix_mappings: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContentControlListItem {
    pub display_text: Option<String>,
    pub value: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DateMetadata {
    pub format: Option<String>,
    pub language_id: Option<String>,
    pub calendar: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ContentControlKind {
    Text,
    RichText,
    Date,
    DropDownList,
    ComboBox,
    CheckBox,
    Picture,
    Group,
    RepeatingSection,
    RepeatingSectionItem,
    #[default]
    Unknown,
}

pub(crate) fn content_controls(
    source: &SourceDocument,
) -> impl Iterator<Item = ContentControl<'_>> {
    source
        .node_ids()
        .filter(move |source_id| word(source, *source_id, "sdt"))
        .map(move |source_id| ContentControl {
            source,
            source_id,
            properties_id: child(source, source_id, "sdtPr"),
            content_id: child(source, source_id, "sdtContent"),
        })
}

fn kind(source: &SourceDocument, properties_id: NodeId) -> ContentControlKind {
    for id in source.children(properties_id) {
        if word(source, id, "text") {
            return ContentControlKind::Text;
        }
        if word(source, id, "richText") {
            return ContentControlKind::RichText;
        }
        if word(source, id, "date") {
            return ContentControlKind::Date;
        }
        if word(source, id, "dropDownList") {
            return ContentControlKind::DropDownList;
        }
        if word(source, id, "comboBox") {
            return ContentControlKind::ComboBox;
        }
        if word(source, id, "picture") {
            return ContentControlKind::Picture;
        }
        if word(source, id, "group") {
            return ContentControlKind::Group;
        }
        if element(source, id, WORD_2010, "checkbox") {
            return ContentControlKind::CheckBox;
        }
        if element(source, id, WORD_2012, "repeatingSection") {
            return ContentControlKind::RepeatingSection;
        }
        if element(source, id, WORD_2012, "repeatingSectionItem") {
            return ContentControlKind::RepeatingSectionItem;
        }
    }
    ContentControlKind::Unknown
}

fn child(source: &SourceDocument, parent: NodeId, name: &str) -> Option<NodeId> {
    source.children(parent).find(|id| word(source, *id, name))
}

fn word(source: &SourceDocument, id: NodeId, name: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(SourceNodeKind::Element { name: xml_name, .. }) if xml_name.local_name() == name && xml_name.namespace_uri().is_some_and(|uri| WORD.contains(&uri)))
}

fn element(source: &SourceDocument, id: NodeId, namespace: &str, name: &str) -> bool {
    matches!(source.node(id).map(|node| node.kind()), Some(SourceNodeKind::Element { name: xml_name, .. }) if xml_name.local_name() == name && xml_name.namespace_uri() == Some(namespace))
}

#[cfg(test)]
mod tests {
    use crate::{DocxDocument, SourceDocument};

    use super::*;

    const WORD: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

    #[test]
    fn discovers_metadata_content_and_nested_controls_in_source_order() {
        let source = SourceDocument::parse(format!(
            "<x:document xmlns:x=\"{WORD}\" xmlns:y=\"http://schemas.microsoft.com/office/word/2010/wordml\"><x:body><x:sdt><x:sdtPr><x:id x:val=\"123\"/><x:alias x:val=\"Customer Name\"/><x:tag x:val=\"customer_name\"/><x:lock x:val=\"sdtLocked\"/><x:placeholder><x:docPart x:val=\"DefaultName\"/></x:placeholder><x:dataBinding x:storeItemID=\"{{id}}\" x:xpath=\"/root/name\" x:prefixMappings=\"xmlns:a='urn:a'\"/><x:text/></x:sdtPr><x:sdtContent><x:p><x:r><x:t>Acme &amp; Co</x:t></x:r><x:sdt><x:sdtPr><x:dropDownList><x:listItem x:displayText=\"Approved\" x:value=\"approved\"/></x:dropDownList></x:sdtPr><x:sdtContent><x:r><x:t> Approved </x:t></x:r></x:sdtContent></x:sdt></x:p></x:sdtContent></x:sdt><x:p><x:sdt><x:sdtPr><y:checkbox/></x:sdtPr><x:sdtContent><x:r><x:t>Checked</x:t></x:r></x:sdtContent></x:sdt></x:p><x:tbl><x:tr><x:tc><x:sdt><x:sdtContent><x:p><x:r><x:t>Cell</x:t></x:r></x:p></x:sdtContent></x:sdt></x:tc></x:tr></x:tbl><x:sdt><x:unknown/></x:sdt></x:body></x:document>"
        ).into_bytes()).unwrap();
        let document = DocxDocument::new(&source).unwrap();
        let controls: Vec<_> = document.content_controls().collect();

        assert_eq!(controls.len(), 5);
        let properties = controls[0].properties();
        assert_eq!(properties.id, Some(123));
        assert_eq!(properties.alias.as_deref(), Some("Customer Name"));
        assert_eq!(properties.tag.as_deref(), Some("customer_name"));
        assert_eq!(properties.kind, ContentControlKind::Text);
        assert_eq!(properties.lock.as_deref(), Some("sdtLocked"));
        assert_eq!(properties.placeholder.as_deref(), Some("DefaultName"));
        assert_eq!(
            properties.data_binding.unwrap().xpath.as_deref(),
            Some("/root/name")
        );
        assert_eq!(controls[0].visible_text().unwrap(), "Acme & Co Approved ");
        assert_eq!(
            controls[1].properties().items[0].value.as_deref(),
            Some("approved")
        );
        assert_eq!(controls[2].properties().kind, ContentControlKind::CheckBox);
        assert_eq!(controls[3].visible_text().unwrap(), "Cell");
        assert_eq!(controls[4].properties().kind, ContentControlKind::Unknown);
        assert!(
            controls
                .iter()
                .all(|control| source.node(control.source_id()).is_some())
        );
        assert_eq!(
            document.paragraphs().next().unwrap().text().unwrap(),
            "Acme & Co Approved "
        );
        assert_eq!(
            document
                .blocks()
                .filter_map(|block| match block {
                    crate::BodyBlock::Table(table) => Some(table),
                    _ => None,
                })
                .next()
                .unwrap()
                .rows()
                .next()
                .unwrap()
                .cells()
                .next()
                .unwrap()
                .text()
                .unwrap(),
            "Cell"
        );
    }

    #[test]
    fn accepts_missing_optional_content_control_parts() {
        let source = SourceDocument::parse(format!("<document xmlns=\"{WORD}\"><body><sdt/><p><sdt><sdtPr><date><dateFormat val=\"M/d/yyyy\"/><lid val=\"en-US\"/><calendar val=\"gregorian\"/></date></sdtPr></sdt></p></body></document>").into_bytes()).unwrap();
        let controls: Vec<_> = DocxDocument::new(&source)
            .unwrap()
            .content_controls()
            .collect();
        assert_eq!(controls.len(), 2);
        assert_eq!(controls[0].properties_id(), None);
        assert_eq!(controls[0].content_id(), None);
        assert_eq!(controls[0].visible_text().unwrap(), "");
        assert_eq!(
            controls[1].properties().date.unwrap().format.as_deref(),
            Some("M/d/yyyy")
        );
    }
}
