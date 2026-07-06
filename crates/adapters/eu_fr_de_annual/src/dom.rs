//! A minimal, dependency-light XML/HTML-fragment DOM shared by the FR (well-formed
//! HATVP XML) and DE (server-rendered disclosure fragment) sub-adapters. Built on
//! `quick-xml` events (the `us_house` precedent) — NOT `scraper`/`html5ever`, whose
//! link footprint tripped the CI runner (`australia_register` SIGBUS lesson,
//! fixtures `MANIFEST.json`). Text is entity-unescaped; whitespace is preserved so
//! the caller decides trimming/normalization per field.

use std::collections::BTreeMap;

use anyhow::Context as _;
use quick_xml::events::{BytesStart, Event};

/// One element node: tag name, attributes, directly-owned text (concatenated
/// across text events), and child elements in document order.
#[derive(Debug, Clone, Default)]
pub struct Node {
    /// Tag name (local, as written).
    pub name: String,
    /// Attributes, unescaped.
    pub attrs: BTreeMap<String, String>,
    /// Concatenated direct text (unescaped, whitespace preserved).
    pub text: String,
    /// Child elements in document order.
    pub children: Vec<Node>,
}

impl Node {
    /// First child element with `name`.
    #[must_use]
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    /// All child elements with `name`, in order.
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// The `class` attribute, or an empty string.
    #[must_use]
    pub fn class(&self) -> &str {
        self.attrs.get("class").map_or("", String::as_str)
    }

    /// Whether this node has any child ELEMENT (vs a leaf scalar).
    #[must_use]
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

/// Parses XML/HTML-fragment bytes into a synthetic `#root` whose children are the
/// document's top-level elements. UTF-8 lossy + BOM-tolerant.
///
/// # Errors
/// Malformed markup that `quick-xml` cannot tokenize (fail closed).
pub fn parse(bytes: &[u8]) -> anyhow::Result<Node> {
    let text = String::from_utf8_lossy(bytes);
    parse_str(text.trim_start_matches('\u{feff}'))
}

/// Parses a UTF-8 XML/HTML fragment string into a synthetic `#root`.
///
/// # Errors
/// Malformed markup or an unbalanced end tag (fail closed).
pub fn parse_str(s: &str) -> anyhow::Result<Node> {
    let mut reader = quick_xml::Reader::from_str(s);
    let config = reader.config_mut();
    config.check_end_names = false; // tolerate HTML void tags left in a fragment

    let mut stack: Vec<Node> = vec![Node {
        name: "#root".to_owned(),
        ..Node::default()
    }];

    loop {
        match reader.read_event().context("reading markup")? {
            Event::Start(e) => stack.push(start_node(&e)?),
            Event::Empty(e) => {
                let node = start_node(&e)?;
                push_child(&mut stack, node)?;
            }
            Event::Text(e) => {
                let decoded = unescape_entities(&String::from_utf8_lossy(e.as_ref()));
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&decoded);
                }
            }
            Event::CData(e) => {
                let raw = String::from_utf8_lossy(e.as_ref()).into_owned();
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&raw);
                }
            }
            // quick-xml 0.41 emits entity/character references as their own event.
            Event::GeneralRef(e) => {
                let name = String::from_utf8_lossy(e.as_ref());
                let inner = name.trim_start_matches('&').trim_end_matches(';');
                if let Some(top) = stack.last_mut() {
                    if let Some(c) = decode_entity(inner) {
                        top.text.push(c);
                    } else {
                        top.text.push('&');
                        top.text.push_str(inner);
                        top.text.push(';');
                    }
                }
            }
            Event::End(_) => {
                let Some(node) = stack.pop() else {
                    anyhow::bail!("unbalanced end tag in markup");
                };
                push_child(&mut stack, node)?;
            }
            Event::Eof => break,
            _ => {}
        }
    }

    // Collapse any elements left open by lenient (void-tolerant) parsing into the
    // root so nothing is silently dropped.
    while stack.len() > 1 {
        let Some(node) = stack.pop() else { break };
        push_child(&mut stack, node)?;
    }
    stack
        .pop()
        .context("markup produced no root node (fail closed)")
}

fn push_child(stack: &mut [Node], node: Node) -> anyhow::Result<()> {
    let Some(parent) = stack.last_mut() else {
        anyhow::bail!("no parent element for {:?} (fail closed)", node.name);
    };
    parent.children.push(node);
    Ok(())
}

fn start_node(e: &BytesStart) -> anyhow::Result<Node> {
    let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
    let mut attrs = BTreeMap::new();
    for attr in e.attributes() {
        let attr = attr.context("reading attribute")?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let value = unescape_entities(&String::from_utf8_lossy(&attr.value));
        attrs.insert(key, value);
    }
    Ok(Node {
        name,
        attrs,
        text: String::new(),
        children: Vec::new(),
    })
}

/// Trim + collapse every internal whitespace run (including newlines from the
/// source's `[Données non publiées]` redaction blocks) to a single space.
#[must_use]
pub fn normalize_ws(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Decodes the standard XML entities plus numeric character references
/// (`quick-xml` 0.41 no longer unescapes text/attribute events for us). Unknown
/// entities are left verbatim.
fn unescape_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let decoded = after
            .find(';')
            .filter(|&i| i <= 12)
            .and_then(|i| decode_entity(&after[..i]).map(|c| (c, &after[i + 1..])));
        if let Some((c, tail)) = decoded {
            out.push(c);
            rest = tail;
        } else {
            out.push('&');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let code = if let Some(hex) = entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
            {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                entity.strip_prefix('#')?.parse().ok()?
            };
            char::from_u32(code)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_tree_with_text_and_children() {
        let root = parse_str("<a><b>hi</b><b>there</b></a>").unwrap();
        let a = root.child("a").unwrap();
        assert_eq!(a.children_named("b").count(), 2);
        assert_eq!(a.child("b").unwrap().text, "hi");
    }

    #[test]
    fn unescapes_entities_and_preserves_internal_space() {
        let root = parse_str("<m>62 389 &amp; more</m>").unwrap();
        assert_eq!(root.child("m").unwrap().text, "62 389 & more");
    }

    #[test]
    fn normalize_ws_collapses_redaction_whitespace() {
        assert_eq!(
            normalize_ws("\n   [Données non publiées]\n  "),
            "[Données non publiées]"
        );
        assert_eq!(normalize_ws("62 389"), "62 389");
    }

    #[test]
    fn reads_attributes_and_class() {
        let root = parse_str(r#"<ul class="voa_list bt-liste"><li>x</li></ul>"#).unwrap();
        let ul = root.child("ul").unwrap();
        assert_eq!(ul.class(), "voa_list bt-liste");
    }
}
