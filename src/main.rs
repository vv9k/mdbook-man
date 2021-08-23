// src/main.rs
extern crate mdbook;

use comrak::nodes::{AstNode, NodeValue};
use comrak::{parse_document, Arena, ComrakOptions};
use mdbook::renderer::RenderContext;
use mdbook::BookItem;
use roffman::{IntoRoffNode, Roff, RoffNode, RoffText, Roffable, SectionNumber};

use std::io;

fn iter_nodes<'a, F>(node: &'a AstNode<'a>, out: &mut Parser, f: &F)
where
    F: Fn(&'a AstNode<'a>, &mut Parser),
{
    f(node, out);
    for c in node.children() {
        iter_nodes(c, out, f);
    }
}

#[derive(Default)]
struct Parser {
    nodes: Vec<RoffNode>,
    last_md_node: MarkdownNode,
    _heading: Option<RoffText>,
}

impl Parser {
    pub fn update_last_node(&mut self, node: MarkdownNode) {
        self.last_md_node = node;
    }

    pub fn last_node(&self) -> &MarkdownNode {
        &self.last_md_node
    }

    pub fn finalize(self) -> Vec<RoffNode> {
        self.nodes
    }

    pub fn append_roff(&mut self, roff: RoffNode) {
        self.nodes.push(roff);
    }

    pub fn set_heading(&mut self, text: RoffText) {
        self._heading = Some(text);
    }

    pub fn consume_heading(&mut self) -> Option<RoffText> {
        std::mem::replace(&mut self._heading, None)
    }

    pub fn has_heading(&self) -> bool {
        self._heading.is_some()
    }
}

#[derive(Copy, Debug, Clone)]
enum MarkdownNode {
    Heading,
    Paragraph,
    Code,
    CodeBlock,
    Strong,
    Emphasis,
    Link,
    SoftBreak,
    Text,
    List,
    ListItem,

    // fallback
    Empty,
}

impl From<&NodeValue> for MarkdownNode {
    fn from(n: &NodeValue) -> Self {
        match n {
            NodeValue::Heading(_) => MarkdownNode::Heading,
            NodeValue::Paragraph => MarkdownNode::Paragraph,
            NodeValue::CodeBlock(_) => MarkdownNode::CodeBlock,
            NodeValue::Code(_) => MarkdownNode::Code,
            NodeValue::Strong => MarkdownNode::Strong,
            NodeValue::Emph => MarkdownNode::Emphasis,
            NodeValue::Link(_) => MarkdownNode::Link,
            NodeValue::SoftBreak => MarkdownNode::SoftBreak,
            NodeValue::Text(_) => MarkdownNode::Text,
            NodeValue::List(_) => MarkdownNode::List,
            NodeValue::Item(_) => MarkdownNode::ListItem,
            _ => MarkdownNode::Empty,
        }
    }
}

impl Default for MarkdownNode {
    fn default() -> Self {
        MarkdownNode::Empty
    }
}

fn parse_markdown<'a>(text: &'a str, arena: &'a Arena<AstNode<'a>>) -> Vec<RoffNode> {
    let mut parser = Parser::default();
    let root = parse_document(&arena, text, &ComrakOptions::default());

    iter_nodes(root, &mut parser, &|node, parser| {
        let value = &node.data.borrow().value;
        eprintln!("node: {:?}", value);
        match value {
            node
            @
            (NodeValue::Heading(_)
            | NodeValue::Paragraph
            | NodeValue::Strong
            | NodeValue::List(_)
            | NodeValue::Emph) => parser.update_last_node(MarkdownNode::from(node)),
            NodeValue::Link(ref link) => {
                let url = String::from_utf8_lossy(link.url.as_slice());
                let title = String::from_utf8_lossy(link.title.as_slice());
                parser.append_roff(RoffNode::url(title, url));
                parser.update_last_node(MarkdownNode::Link);
            }
            NodeValue::SoftBreak => {
                parser.append_roff("\r\n".into_roff());
                parser.update_last_node(MarkdownNode::SoftBreak);
            }
            NodeValue::Code(code) => {
                let text = String::from_utf8_lossy(code.literal.as_slice());
                parser.append_roff(text.roff().italic().into_roff());
                parser.update_last_node(MarkdownNode::Code);
            }
            NodeValue::CodeBlock(ref block) => {
                let text = String::from_utf8_lossy(block.literal.as_slice());
                let info = String::from_utf8_lossy(block.info.as_slice());
                let title = if !info.is_empty() {
                    Some(info.roff().bold())
                } else {
                    None
                };
                let para = RoffNode::indented_paragraph(
                    [
                        if let Some(title) = title {
                            title.bold()
                        } else {
                            "".roff()
                        }
                        .into_roff(),
                        RoffNode::indented_paragraph(
                            [RoffNode::example([text.as_ref(), "\n"])],
                            Some(4),
                            None::<&str>,
                        ),
                    ],
                    Some(2),
                    None::<&str>,
                );
                parser.append_roff(para);
                parser.update_last_node(MarkdownNode::CodeBlock);
            }
            NodeValue::Text(ref text) => {
                let text = String::from_utf8_lossy(text);
                match parser.last_node() {
                    MarkdownNode::Heading if !parser.has_heading() => {
                        parser.set_heading(text.roff().bold());
                        return;
                    }
                    MarkdownNode::Heading if parser.has_heading() => {
                        let heading = parser.consume_heading().unwrap();
                        parser.append_roff(RoffNode::tagged_paragraph([text], heading));
                    }

                    MarkdownNode::Paragraph => {
                        parser.append_roff(RoffNode::paragraph([text]));
                    }
                    MarkdownNode::Emphasis => {
                        parser.append_roff(text.roff().italic().into_roff());
                    }
                    MarkdownNode::Strong => {
                        parser.append_roff(text.roff().bold().into_roff());
                    }
                    _ => {
                        parser.append_roff(text.into_roff());
                    }
                }
                parser.update_last_node(MarkdownNode::Text);
            }
            NodeValue::Document => {}
            n if parser.has_heading() => {
                let heading = parser.consume_heading().unwrap();
                parser.append_roff("\n".into_roff());
                parser.append_roff(heading.bold().into_roff());
                parser.append_roff("\n".into_roff());
                parser.update_last_node(MarkdownNode::Text);
            }
            n => {
                eprintln!("unhandled node: {:?}", n);
                parser.update_last_node(MarkdownNode::Empty);
            }
        }
    });
    let x = parser.finalize();
    eprintln!("roffs: {:#?}", x);
    x
}

fn main() {
    let mut stdin = io::stdin();
    let ctx = RenderContext::from_json(&mut stdin).unwrap();
    let arena = Arena::new();

    let mut page = Roff::new(
        ctx.config.book.title.unwrap_or_default(),
        SectionNumber::Miscellaneous,
    );

    for item in ctx.book.iter() {
        if let BookItem::Chapter(ref ch) = *item {
            let parsed = parse_markdown(ch.content.as_str(), &arena);
            page = page.section(ch.name.as_str(), parsed);
        }
    }

    println!("{}", page.to_string().unwrap());
}
