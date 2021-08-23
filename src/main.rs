// src/main.rs
extern crate mdbook;

use comrak::nodes::{AstNode, NodeValue};
use comrak::{parse_document, Arena, ComrakOptions};
use mdbook::renderer::RenderContext;
use mdbook::BookItem;
use roffman::{IntoRoffNode, Roff, RoffNode, Roffable, SectionNumber};
use serde::{Deserialize, Serialize};

use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

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
    List,
    ListItem,
    LineBreak,
    Image,

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
            NodeValue::LineBreak => MarkdownNode::LineBreak,
            NodeValue::Image(_) => MarkdownNode::Image,
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
    let root = parse_document(arena, text, &ComrakOptions::default());

    iter_nodes(root, &mut parser, &|node, parser| {
        let value = &node.data.borrow().value;
        match value {
            NodeValue::Link(ref link) | NodeValue::Image(ref link) => {
                let url = String::from_utf8_lossy(link.url.as_slice());
                let title = String::from_utf8_lossy(link.title.as_slice());
                parser.append_roff(RoffNode::url(title, url));
            }
            NodeValue::Code(code) => {
                let text = String::from_utf8_lossy(code.literal.as_slice());
                parser.append_roff("`".into_roff());
                parser.append_roff(text.roff().italic().into_roff());
                parser.append_roff("`".into_roff());
            }
            NodeValue::CodeBlock(ref block) => {
                let text = String::from_utf8_lossy(block.literal.as_slice());
                let info = String::from_utf8_lossy(block.info.as_slice());
                let title = if !info.is_empty() {
                    Some(info.roff().bold())
                } else {
                    None
                };
                let para = RoffNode::nested([RoffNode::indented_paragraph(
                    [RoffNode::linebreak(), RoffNode::example([text.as_ref()])],
                    Some(2),
                    title,
                )]);
                parser.append_roff(para);
            }
            NodeValue::Text(ref text) => {
                let text = String::from_utf8_lossy(text);
                match parser.last_node() {
                    MarkdownNode::Heading => {
                        parser.append_roff(RoffNode::linebreak());
                        parser.append_roff(RoffNode::linebreak());
                        parser.append_roff(text.roff().bold().into_roff());
                        parser.append_roff(RoffNode::linebreak());
                        parser.append_roff("=".repeat(text.len() + 2).into_roff());
                        parser.append_roff(RoffNode::linebreak());
                        return;
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
                    MarkdownNode::ListItem => {
                        parser.append_roff(text.into_roff());
                        parser.append_roff(RoffNode::linebreak());
                    }
                    _ => {
                        parser.append_roff(text.into_roff());
                    }
                }
            }
            NodeValue::LineBreak => {
                parser.append_roff(RoffNode::linebreak());
            }
            _ => {}
        }

        parser.update_last_node(MarkdownNode::from(value));
    });
    parser.finalize()
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ManOutputConfiguration {
    pub output_dir: Option<PathBuf>,
    #[serde(default)]
    pub split_chapters: bool,
}

fn main() {
    let mut stdin = io::stdin();
    let ctx = RenderContext::from_json(&mut stdin).unwrap();
    let arena = Arena::new();
    let cfg: ManOutputConfiguration = ctx
        .config
        .get_deserialized_opt("output.man")
        .ok()
        .flatten()
        .unwrap_or_default();

    if !cfg.split_chapters {
        let title = ctx.config.book.title.unwrap_or_default();
        let mut page = Roff::new(&title, SectionNumber::Miscellaneous);

        for item in ctx.book.iter() {
            if let BookItem::Chapter(ref ch) = *item {
                let parsed = parse_markdown(ch.content.as_str(), &arena);
                page = page.section(ch.name.as_str(), parsed);
            }
        }

        let page = page.to_string().unwrap();

        if let Some(path) = cfg.output_dir {
            if !path.exists() {
                fs::create_dir_all(&path).unwrap();
            }
            fs::write(path.join("book.man"), page).unwrap()
        } else {
            println!("{}", page)
        }
    } else {
        let mut pages = vec![];
        for item in ctx.book.iter() {
            if let BookItem::Chapter(ref ch) = *item {
                let mut page = Roff::new(ch.name.as_str(), SectionNumber::Miscellaneous);
                let parsed = parse_markdown(ch.content.as_str(), &arena);
                page = page.section(ch.name.as_str(), parsed);
                pages.push(page);
            }
        }

        for (i, page) in pages.iter().enumerate() {
            let page = page.to_string().unwrap();

            if let Some(path) = &cfg.output_dir {
                if !path.exists() {
                    fs::create_dir_all(&path).unwrap();
                }
                fs::write(path.join(format!("chapter{}.man", i)), page).unwrap()
            } else {
                println!("{}", page)
            }
        }
    }
}
