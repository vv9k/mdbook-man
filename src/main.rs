// src/main.rs
extern crate mdbook;

use comrak::arena_tree::NodeEdge;
use comrak::nodes::{Ast, AstNode, NodeValue};
use comrak::{format_html, parse_document, Arena, ComrakOptions};
use mdbook::renderer::RenderContext;
use mdbook::{book::Chapter, BookItem};
use roffman::{FontStyle, IntoRoffNode, Roff, RoffNode, RoffText, Roffable, SectionNumber};

use std::borrow::Cow;
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
        // eprintln!("node: {:?}", value);
        match value {
            node
            @
            (NodeValue::Heading(_)
            | NodeValue::Paragraph
            | NodeValue::Code(_)
            | NodeValue::Strong
            | NodeValue::Emph) => parser.update_last_node(MarkdownNode::from(node)),
            NodeValue::Link(ref link) => {
                let url = String::from_utf8_lossy(link.url.as_slice());
                let title = String::from_utf8_lossy(link.title.as_slice());
                parser.append_roff(RoffNode::url(title, url));
                parser.update_last_node(MarkdownNode::Link);
            }
            NodeValue::SoftBreak => {
                // parser.append_roff("\r".into_roff());
                // parser.update_last_node(MarkdownNode::SoftBreak);
            }
            NodeValue::CodeBlock(ref block) => {
                let text = String::from_utf8_lossy(block.literal.as_slice());
                eprintln!("```{}```\n\n", text);
                let info = String::from_utf8_lossy(block.info.as_slice());
                let title = if !info.is_empty() {
                    Some(info.roff().bold())
                } else {
                    None
                };
                let para = RoffNode::indented_paragraph(
                    [RoffNode::example([text.as_ref(), "\n"])],
                    Some(2),
                    title,
                );
                parser.append_roff(para);
                parser.update_last_node(MarkdownNode::CodeBlock);
            }
            NodeValue::Text(ref text) => {
                let text = String::from_utf8_lossy(text);
                match parser.last_node() {
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
            n => {
                eprintln!("unhandled node: {:?}", n);
                parser.update_last_node(MarkdownNode::Empty);
            }
        }
    });
    parser.finalize()
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
