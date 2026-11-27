#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Element {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Element>,
    pub text: String,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    pub fn required_attr(&self, name: &str) -> Result<&str, String> {
        let value = self
            .attr(name)
            .ok_or_else(|| format!("<{}> is missing required attribute {name:?}", self.name))?
            .trim();
        if value.is_empty() {
            return Err(format!(
                "<{}> attribute {name:?} must not be empty",
                self.name
            ));
        }
        Ok(value)
    }

    pub fn check_allowed_attrs(&self, allowed: &[&str]) -> Result<(), String> {
        for (key, _) in &self.attributes {
            if !allowed.contains(&key.as_str()) {
                return Err(format!("Unexpected attribute {key:?} on <{}>", self.name));
            }
        }
        Ok(())
    }

    pub fn child(&self, name: &str) -> Result<Option<&Element>, String> {
        let mut matches = self.children.iter().filter(|child| child.name == name);
        let first = matches.next();
        if matches.next().is_some() {
            return Err(format!("<{}> must contain at most one <{name}>", self.name));
        }
        Ok(first)
    }

    pub fn required_child(&self, name: &str) -> Result<&Element, String> {
        self.child(name)?
            .ok_or_else(|| format!("<{}> is missing required <{name}> element", self.name))
    }

    pub fn check_allowed_children(&self, allowed: &[&str]) -> Result<(), String> {
        for child in &self.children {
            if !allowed.contains(&child.name.as_str()) {
                return Err(format!(
                    "Unexpected <{}> inside <{}>",
                    child.name, self.name
                ));
            }
        }
        Ok(())
    }

    pub fn normalized_text(&self) -> Result<String, String> {
        if !self.children.is_empty() {
            return Err(format!(
                "<{}> must contain text only, not child elements",
                self.name
            ));
        }
        Ok(self.text.split_whitespace().collect::<Vec<_>>().join(" "))
    }
}

pub(super) fn parse(input: &str) -> Result<Element, String> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut parser = Parser { input, pos: 0 };
    parser.skip_trivia()?;
    if parser.eof() {
        return Err("XML document is empty".into());
    }
    let root = parser.parse_element()?;
    parser.skip_trivia()?;
    if !parser.eof() {
        return Err(parser.error("Unexpected content after root element"));
    }
    Ok(root)
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn line_col(&self) -> (usize, usize) {
        let before = &self.input[..self.pos];
        let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
        let col = before
            .rsplit('\n')
            .next()
            .map_or(1, |segment| segment.chars().count() + 1);
        (line, col)
    }

    fn error(&self, message: &str) -> String {
        let (line, col) = self.line_col();
        format!("{message} (line {line}, column {col})")
    }

    fn skip_ws(&mut self) {
        let trimmed = self.rest().trim_start_matches([' ', '\t', '\r', '\n']);
        self.pos = self.input.len() - trimmed.len();
    }

    fn skip_trivia(&mut self) -> Result<(), String> {
        loop {
            self.skip_ws();
            if self.rest().starts_with("<!--") {
                let end = self
                    .rest()
                    .find("-->")
                    .ok_or_else(|| self.error("Unclosed XML comment"))?;
                if self.rest()[4..end].contains("--") {
                    return Err(self.error("'--' is not allowed inside an XML comment"));
                }
                self.pos += end + 3;
            } else if self.rest().starts_with("<?") {
                let end = self
                    .rest()
                    .find("?>")
                    .ok_or_else(|| self.error("Unclosed XML processing instruction"))?;
                self.pos += end + 2;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn parse_name(&mut self) -> Result<String, String> {
        let rest = self.rest();
        let mut chars = rest.char_indices();
        let Some((_, first)) = chars.next() else {
            return Err(self.error("Expected XML name"));
        };
        if !(first.is_alphabetic() || first == '_' || first == ':') {
            return Err(self.error("Invalid XML name start character"));
        }
        let mut end = first.len_utf8();
        for (idx, ch) in chars {
            if ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':') {
                end = idx + ch.len_utf8();
            } else {
                break;
            }
        }
        let name = rest[..end].to_owned();
        self.pos += end;
        Ok(name)
    }

    fn parse_element(&mut self) -> Result<Element, String> {
        if !self.rest().starts_with('<') {
            return Err(self.error("Expected '<' to start element"));
        }
        self.pos += 1;
        let name = self.parse_name()?;
        let mut attributes = Vec::new();
        loop {
            self.skip_ws();
            if self.rest().starts_with("/>") {
                self.pos += 2;
                return Ok(Element {
                    name,
                    attributes,
                    children: Vec::new(),
                    text: String::new(),
                });
            }
            if self.rest().starts_with('>') {
                self.pos += 1;
                break;
            }
            let attr_name = self.parse_name()?;
            if attributes
                .iter()
                .any(|(existing, _)| existing == &attr_name)
            {
                return Err(self.error(&format!("Duplicate attribute {attr_name:?} on <{name}>")));
            }
            self.skip_ws();
            if !self.rest().starts_with('=') {
                return Err(self.error(&format!("Expected '=' after attribute {attr_name:?}")));
            }
            self.pos += 1;
            self.skip_ws();
            let quote = self
                .rest()
                .chars()
                .next()
                .filter(|c| *c == '"' || *c == '\'')
                .ok_or_else(|| self.error("Expected quoted attribute value"))?;
            self.pos += 1;
            let close = self
                .rest()
                .find(quote)
                .ok_or_else(|| self.error("Unclosed attribute value"))?;
            let raw = &self.rest()[..close];
            if raw.contains('<') {
                return Err(self.error("'<' is not allowed in attribute values"));
            }
            let value = unescape(raw).map_err(|msg| self.error(&msg))?;
            self.pos += close + 1;
            attributes.push((attr_name, value));
        }

        let mut children = Vec::new();
        let mut text = String::new();
        loop {
            if self.eof() {
                return Err(self.error(&format!("Unclosed <{name}> element")));
            }
            if self.rest().starts_with("</") {
                self.pos += 2;
                let close_name = self.parse_name()?;
                self.skip_ws();
                if close_name != name || !self.rest().starts_with('>') {
                    return Err(self.error(&format!(
                        "Mismatched closing tag </{close_name}> for <{name}>"
                    )));
                }
                self.pos += 1;
                break;
            } else if self.rest().starts_with("<!--") {
                let end = self
                    .rest()
                    .find("-->")
                    .ok_or_else(|| self.error("Unclosed XML comment"))?;
                if self.rest()[4..end].contains("--") {
                    return Err(self.error("'--' is not allowed inside an XML comment"));
                }
                self.pos += end + 3;
            } else if self.rest().starts_with("<![CDATA[") {
                let end = self
                    .rest()
                    .find("]]>")
                    .ok_or_else(|| self.error("Unclosed CDATA section"))?;
                text.push_str(&self.rest()[9..end]);
                self.pos += end + 3;
            } else if self.rest().starts_with("<?") {
                let end = self
                    .rest()
                    .find("?>")
                    .ok_or_else(|| self.error("Unclosed XML processing instruction"))?;
                self.pos += end + 2;
            } else if self.rest().starts_with('<') {
                children.push(self.parse_element()?);
            } else {
                let next_lt = self.rest().find('<').unwrap_or(self.rest().len());
                let raw = &self.rest()[..next_lt];
                if raw.contains("]]>") {
                    return Err(self.error("']]>' is not allowed in element text"));
                }
                let decoded = unescape(raw).map_err(|msg| self.error(&msg))?;
                text.push_str(&decoded);
                self.pos += next_lt;
            }
        }
        if !children.is_empty() && !text.trim().is_empty() {
            return Err(self.error(&format!(
                "Mixed text and child elements are not allowed in <{name}>"
            )));
        }
        Ok(Element {
            name,
            attributes,
            children,
            text,
        })
    }
}

fn unescape(raw: &str) -> Result<String, String> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let semi = after
            .find(';')
            .ok_or_else(|| "Unclosed XML entity reference".to_owned())?;
        let entity = &after[..semi];
        let decoded = match entity {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                let code = u32::from_str_radix(&entity[2..], 16)
                    .map_err(|_| format!("Invalid hex character reference '&{entity};'"))?;
                char::from_u32(code)
                    .ok_or_else(|| format!("Invalid Unicode scalar in '&{entity};'"))?
            }
            _ if entity.starts_with('#') => {
                let code = entity[1..]
                    .parse::<u32>()
                    .map_err(|_| format!("Invalid numeric character reference '&{entity};'"))?;
                char::from_u32(code)
                    .ok_or_else(|| format!("Invalid Unicode scalar in '&{entity};'"))?
            }
            _ => return Err(format!("Unknown XML entity '&{entity};'")),
        };
        out.push(decoded);
        rest = &after[semi + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

pub(super) fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

pub(super) fn escape_attr(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}
