use super::*;
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use quick_xml::{
    events::{BytesEnd, BytesStart, BytesText, Event},
    Writer,
};

fn text(value: &Value) -> Result<&str> {
    match value {
        Value::Text(value) => Ok(value),
        _ => Err("Expected a string".into()),
    }
}

fn tagged(name: &str, values: impl IntoIterator<Item = Value>) -> Value {
    let mut list = vec![Value::Symbol(name.into())];
    list.extend(values);
    Value::List(list)
}

fn parts<'a>(value: &'a Value, tag: &str) -> Result<&'a [Value]> {
    if let Value::List(items) = value {
        if matches!(items.first(), Some(Value::Symbol(name)) if name.is(tag)) {
            return Ok(&items[1..]);
        }
    }
    Err(format!("Expected a {tag} value").into())
}

impl Interpreter {
    pub(super) fn data_builtin(
        &mut self,
        builtin: Builtin,
        args: Vec<Value>,
        depth: usize,
    ) -> Result<Step> {
        builtin.arguments().check(builtin.name(), &args)?;
        let value = match builtin {
            JsonParse => {
                let input = text(&args[0])?;
                self.check_data_size(input)?;
                let json = serde_json::from_str(input).map_err(|error| format!("JSON: {error}"))?;
                self.json_value(json, depth + 1)?
            }
            JsonStringify => {
                let json = self.encode_json(&args[0], depth + 1)?;
                let output =
                    serde_json::to_string(&json).map_err(|error| format!("JSON: {error}"))?;
                self.check_data_size(&output)?;
                Value::Text(output)
            }
            JsonGet | JsonSet | JsonRemove => self.json_edit(builtin, args, depth + 1)?,
            XmlParse => {
                let input = text(&args[0])?;
                self.check_data_size(input)?;
                let document =
                    roxmltree::Document::parse(input).map_err(|error| format!("XML: {error}"))?;
                self.xml_node(document.root(), depth + 1)?
            }
            HtmlParse => {
                let input = text(&args[0])?;
                self.check_data_size(input)?;
                let dom = parse_document(RcDom::default(), Default::default()).one(input);
                self.html_node(&dom.document, depth + 1)?
            }
            XmlStringify | HtmlStringify => {
                let html = builtin == HtmlStringify;
                let mut writer = Writer::new(Vec::new());
                self.write_node(&args[0], &mut writer, html, None, depth + 1)?;
                let output = String::from_utf8(writer.into_inner())
                    .map_err(|error| format!("Markup: {error}"))?;
                self.check_data_size(&output)?;
                if !html {
                    roxmltree::Document::parse(&output).map_err(|error| format!("XML: {error}"))?;
                }
                Value::Text(output)
            }
            _ => unreachable!(),
        };
        Ok(Step::Value(value))
    }

    fn check_data_size(&self, text: &str) -> Result<()> {
        if text.len() > limits::OUTPUT_BYTES {
            Err("Data size limit reached (128 KiB)".into())
        } else {
            Ok(())
        }
    }

    fn json_value(&mut self, value: serde_json::Value, depth: usize) -> Result<Value> {
        self.tick(depth)?;
        use serde_json::Value as J;
        Ok(match value {
            J::Null => Value::Symbol("null".into()),
            J::Bool(boolean) => Value::Bool(boolean),
            J::String(string) => Value::Text(string),
            J::Number(number) => {
                let n = number.as_f64().ok_or("JSON number cannot be represented")?;
                if !n.is_finite() {
                    return Err("JSON number must be finite".into());
                }
                Value::Number(n)
            }
            J::Array(array) => {
                if array.len() >= limits::LIST_ITEMS {
                    return Err("List size limit reached".into());
                }
                let values = array
                    .into_iter()
                    .map(|value| self.json_value(value, depth + 1))
                    .collect::<Result<Vec<_>>>()?;
                tagged("array", values)
            }
            J::Object(object) => {
                if object.len() >= limits::LIST_ITEMS {
                    return Err("List size limit reached".into());
                }
                let pairs = object
                    .into_iter()
                    .map(|(key, value)| {
                        Ok(Value::List(vec![
                            Value::Text(key),
                            self.json_value(value, depth + 1)?,
                        ]))
                    })
                    .collect::<Result<Vec<_>>>()?;
                tagged("object", pairs)
            }
        })
    }

    fn encode_json(&mut self, value: &Value, depth: usize) -> Result<serde_json::Value> {
        self.tick(depth)?;
        use serde_json::Value as J;
        Ok(match value {
            Value::Symbol(symbol) if symbol.is("null") => J::Null,
            Value::Bool(boolean) => J::Bool(*boolean),
            Value::Text(text) => J::String(text.clone()),
            Value::Number(number) => {
                if !number.is_finite() {
                    return Err("JSON number must be finite".into());
                }
                J::Number(
                    number
                        .to_string()
                        .parse()
                        .map_err(|error| format!("JSON number: {error}"))?,
                )
            }
            Value::List(values) if matches!(values.first(), Some(Value::Symbol(name)) if name.is("array")) => {
                J::Array(
                    values[1..]
                        .iter()
                        .map(|value| self.encode_json(value, depth + 1))
                        .collect::<Result<_>>()?,
                )
            }
            Value::List(_) => {
                let mut object = serde_json::Map::new();
                for pair in parts(value, "object")? {
                    let Value::List(pair) = pair else {
                        return Err("JSON object needs (string value) pairs".into());
                    };
                    if pair.len() != 2 {
                        return Err("JSON object needs (string value) pairs".into());
                    }
                    let key = text(&pair[0])?.to_string();
                    let value = self.encode_json(&pair[1], depth + 1)?;
                    if object.insert(key, value).is_some() {
                        return Err("Duplicate JSON object key".into());
                    }
                }
                J::Object(object)
            }
            _ => {
                return Err(
                    "Expected JSON data: object, array, null, boolean, string, or number".into(),
                )
            }
        })
    }

    fn json_edit(&mut self, builtin: Builtin, args: Vec<Value>, depth: usize) -> Result<Value> {
        let mut json = self.encode_json(&args[0], depth)?;
        let value = if builtin == JsonSet {
            Some(self.encode_json(&args[2], depth)?)
        } else {
            None
        };
        let found = match &mut json {
            serde_json::Value::Object(object) => {
                let key = text(&args[1])?;
                if let Some(value) = value {
                    object.insert(key.to_string(), value);
                    None
                } else if builtin == JsonGet {
                    Some(object.get(key).cloned().ok_or("JSON key not found")?)
                } else {
                    object.remove(key).ok_or("JSON key not found")?;
                    None
                }
            }
            serde_json::Value::Array(array) => {
                let Value::Number(index) = &args[1] else {
                    return Err("JSON array index must be an integer".into());
                };
                if *index < 0.0 || index.fract() != 0.0 || *index >= array.len() as f64 {
                    return Err("JSON array index out of bounds".into());
                }
                let index = *index as usize;
                if let Some(value) = value {
                    array[index] = value;
                    None
                } else if builtin == JsonGet {
                    Some(array[index].clone())
                } else {
                    array.remove(index);
                    None
                }
            }
            _ => return Err("JSON access requires an object or array".into()),
        };
        self.json_value(found.unwrap_or(json), depth)
    }

    fn xml_node(&mut self, node: roxmltree::Node<'_, '_>, depth: usize) -> Result<Value> {
        self.tick(depth)?;
        let value = match node.node_type() {
            roxmltree::NodeType::Root => {
                let children = node
                    .children()
                    .map(|child| self.xml_node(child, depth + 1))
                    .collect::<Result<Vec<_>>>()?;
                if children.len() >= limits::LIST_ITEMS {
                    return Err("List size limit reached".into());
                }
                tagged("document", children)
            }
            roxmltree::NodeType::Element => {
                // Namespace declarations are retained so expanded names can be serialized again.
                let mut attributes = Vec::new();
                for namespace in node.namespaces() {
                    attributes.push(Value::List(vec![
                        Value::Text(
                            namespace
                                .name()
                                .map_or_else(|| "xmlns".into(), |prefix| format!("xmlns:{prefix}")),
                        ),
                        Value::Text(namespace.uri().into()),
                    ]));
                }
                let qualified = |namespace: Option<&str>, local: &str| {
                    namespace
                        .and_then(|uri| node.lookup_prefix(uri))
                        .filter(|prefix| !prefix.is_empty())
                        .map_or_else(|| local.to_string(), |prefix| format!("{prefix}:{local}"))
                };
                for attribute in node.attributes() {
                    let name = if let Some(uri) = attribute.namespace() {
                        let prefix = if uri == "http://www.w3.org/XML/1998/namespace" {
                            Some("xml")
                        } else {
                            node.namespaces().find_map(|namespace| {
                                (namespace.uri() == uri).then(|| namespace.name()).flatten()
                            })
                        }
                        .ok_or("XML attribute namespace requires a prefix")?;
                        format!("{prefix}:{}", attribute.name())
                    } else {
                        attribute.name().to_string()
                    };
                    attributes.push(Value::List(vec![
                        Value::Text(name),
                        Value::Text(attribute.value().into()),
                    ]));
                }
                if attributes.len() > limits::LIST_ITEMS {
                    return Err("List size limit reached".into());
                }
                let mut values = vec![
                    Value::Text(qualified(
                        node.tag_name().namespace(),
                        node.tag_name().name(),
                    )),
                    Value::List(attributes),
                ];
                for child in node.children() {
                    values.push(self.xml_node(child, depth + 1)?);
                }
                if values.len() >= limits::LIST_ITEMS {
                    return Err("List size limit reached".into());
                }
                tagged("element", values)
            }
            roxmltree::NodeType::Text => Value::Text(node.text().unwrap_or_default().into()),
            roxmltree::NodeType::Comment => tagged(
                "comment",
                [Value::Text(node.text().unwrap_or_default().into())],
            ),
            roxmltree::NodeType::PI => {
                let instruction = node.pi().ok_or("Invalid XML processing instruction")?;
                tagged(
                    "instruction",
                    [
                        Value::Text(instruction.target.into()),
                        Value::Text(instruction.value.unwrap_or_default().into()),
                    ],
                )
            }
        };
        Ok(value)
    }

    fn html_node(&mut self, node: &Handle, depth: usize) -> Result<Value> {
        self.tick(depth)?;
        let mut values = Vec::new();
        let tag = match &node.data {
            NodeData::Document => "document",
            NodeData::Element {
                name,
                attrs,
                template_contents,
                ..
            } => {
                values.push(Value::Text(name.local.to_string()));
                if attrs.borrow().len() > limits::LIST_ITEMS {
                    return Err("List size limit reached".into());
                }
                let attributes = attrs
                    .borrow()
                    .iter()
                    .map(|attribute| {
                        Value::List(vec![
                            Value::Text(attribute.name.prefix.as_ref().map_or_else(
                                || attribute.name.local.to_string(),
                                |prefix| format!("{prefix}:{}", attribute.name.local),
                            )),
                            Value::Text(attribute.value.to_string()),
                        ])
                    })
                    .collect();
                values.push(Value::List(attributes));
                if let Some(contents) = template_contents.borrow().as_ref() {
                    for child in contents.children.borrow().iter() {
                        values.push(self.html_node(child, depth + 1)?);
                    }
                }
                "element"
            }
            NodeData::Text { contents } => return Ok(Value::Text(contents.borrow().to_string())),
            NodeData::Comment { contents } => {
                return Ok(tagged("comment", [Value::Text(contents.to_string())]))
            }
            NodeData::Doctype {
                name,
                public_id,
                system_id,
            } => {
                return Ok(tagged(
                    "doctype",
                    [
                        Value::Text(name.to_string()),
                        Value::Text(public_id.to_string()),
                        Value::Text(system_id.to_string()),
                    ],
                ))
            }
            NodeData::ProcessingInstruction { target, contents } => {
                return Ok(tagged(
                    "instruction",
                    [
                        Value::Text(target.to_string()),
                        Value::Text(contents.to_string()),
                    ],
                ))
            }
        };
        for child in node.children.borrow().iter() {
            values.push(self.html_node(child, depth + 1)?);
        }
        if values.len() >= limits::LIST_ITEMS {
            return Err("List size limit reached".into());
        }
        Ok(tagged(tag, values))
    }

    fn write_node(
        &mut self,
        value: &Value,
        writer: &mut Writer<Vec<u8>>,
        html: bool,
        parent: Option<&str>,
        depth: usize,
    ) -> Result<()> {
        self.tick(depth)?;
        if let Value::Text(string) = value {
            if html
                && parent.is_some_and(|tag| {
                    [
                        "script",
                        "style",
                        "xmp",
                        "iframe",
                        "noembed",
                        "noframes",
                        "plaintext",
                    ]
                    .contains(&tag)
                })
            {
                writer.get_mut().extend_from_slice(string.as_bytes());
            } else {
                writer
                    .write_event(Event::Text(BytesText::new(string)))
                    .map_err(|error| format!("Markup: {error}"))?;
            }
        } else if let Ok(children) = parts(value, "document") {
            for child in children {
                self.write_node(child, writer, html, None, depth + 1)?;
            }
        } else if let Ok(values) = parts(value, "element") {
            if values.len() < 2 {
                return Err("Element needs a name, attribute list, and optional children".into());
            }
            let name = text(&values[0])?;
            if name.is_empty()
                || name
                    .chars()
                    .any(|c| c.is_control() || c.is_whitespace() || "<>/\"'=&".contains(c))
            {
                return Err("Invalid element name".into());
            }
            let mut start = BytesStart::new(name);
            let Value::List(attributes) = &values[1] else {
                return Err("Expected an attribute list".into());
            };
            let mut names = HashSet::new();
            for attribute in attributes {
                let Value::List(pair) = attribute else {
                    return Err("Attribute needs a (name value) pair".into());
                };
                if pair.len() != 2 {
                    return Err("Attribute needs a (name value) pair".into());
                }
                let key = text(&pair[0])?;
                if key.is_empty()
                    || key
                        .chars()
                        .any(|c| c.is_control() || c.is_whitespace() || "<>/\"'=&".contains(c))
                    || !names.insert(key)
                {
                    return Err("Invalid or duplicate attribute name".into());
                }
                start.push_attribute((key, text(&pair[1])?));
            }
            let void = html
                && [
                    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta",
                    "param", "source", "track", "wbr",
                ]
                .contains(&name);
            if void && values.len() > 2 {
                return Err("HTML void elements cannot have children".into());
            }
            writer
                .write_event(Event::Start(start))
                .map_err(|error| format!("Markup: {error}"))?;
            for child in &values[2..] {
                self.write_node(child, writer, html, Some(name), depth + 1)?;
            }
            if !void {
                writer
                    .write_event(Event::End(BytesEnd::new(name)))
                    .map_err(|error| format!("Markup: {error}"))?;
            }
        } else if let Ok(values) = parts(value, "comment") {
            arity("comment node", values, 1)?;
            let comment = text(&values[0])?;
            if comment.contains("--") || comment.ends_with('-') {
                return Err("Invalid markup comment".into());
            }
            writer
                .write_event(Event::Comment(BytesText::from_escaped(comment)))
                .map_err(|error| format!("Markup: {error}"))?;
        } else if let Ok(values) = parts(value, "instruction") {
            arity("instruction node", values, 2)?;
            let target = text(&values[0])?;
            let body = text(&values[1])?;
            if target.is_empty()
                || target.chars().any(char::is_whitespace)
                || target.contains('?')
                || body.contains("?>")
            {
                return Err("Invalid processing instruction".into());
            }
            writer
                .get_mut()
                .extend_from_slice(format!("<?{target} {body}?>").as_bytes());
        } else if html {
            let values = parts(value, "doctype")?;
            arity("doctype node", values, 3)?;
            let name = text(&values[0])?;
            let public = text(&values[1])?;
            let system = text(&values[2])?;
            if name.is_empty()
                || name
                    .chars()
                    .any(|c| c.is_control() || c.is_whitespace() || "<>\"'".contains(c))
            {
                return Err("Invalid HTML doctype name".into());
            }
            fn identifier(value: &str) -> Result<String> {
                if value.contains('\0') {
                    return Err("Invalid HTML doctype identifier".into());
                }
                if !value.contains('"') {
                    Ok(format!("\"{value}\""))
                } else if !value.contains('\'') {
                    Ok(format!("'{value}'"))
                } else {
                    Err("HTML doctype identifier cannot contain both quote characters".into())
                }
            }
            let declaration = if !public.is_empty() {
                format!(
                    "<!DOCTYPE {name} PUBLIC {} {}>",
                    identifier(public)?,
                    identifier(system)?
                )
            } else if !system.is_empty() {
                format!("<!DOCTYPE {name} SYSTEM {}>", identifier(system)?)
            } else {
                format!("<!DOCTYPE {name}>")
            };
            writer.get_mut().extend_from_slice(declaration.as_bytes());
        } else {
            return Err(
                "Expected a document, element, text, comment, or processing instruction".into(),
            );
        }
        if writer.get_ref().len() > limits::OUTPUT_BYTES {
            return Err("Data size limit reached (128 KiB)".into());
        }
        Ok(())
    }
}
