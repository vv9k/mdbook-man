extern crate mdbook;

use comrak::{
    nodes::{AstNode, NodeValue},
    parse_document, Arena, ComrakOptions,
};
use mdbook::{renderer::RenderContext, BookItem};
use roffman::{IntoRoffNode, Roff, RoffNode, Roffable, SectionNumber};

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

    pub fn append_roff(&mut self, roff: impl IntoRoffNode) {
        self.nodes.push(roff.into_roff());
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

fn markdown_to_roff<'a>(text: &'a str, arena: &'a Arena<AstNode<'a>>) -> Vec<RoffNode> {
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

pub fn mdbook_to_roff(ctx: &RenderContext) -> Roff {
    let arena = Arena::new();
    let title = ctx.config.book.title.as_deref().unwrap_or_default();
    let mut page = Roff::new(&title, SectionNumber::Miscellaneous);

    for item in ctx.book.iter() {
        if let BookItem::Chapter(ref ch) = *item {
            let parsed = markdown_to_roff(ch.content.as_str(), &arena);
            page = page.section(ch.name.as_str(), parsed);
        }
    }

    page
}

pub fn mdbook_to_roff_chapters(ctx: &RenderContext) -> Vec<Roff> {
    let arena = Arena::new();
    let mut pages = vec![];
    for item in ctx.book.iter() {
        if let BookItem::Chapter(ref ch) = *item {
            let mut page = Roff::new(ch.name.as_str(), SectionNumber::Miscellaneous);
            let parsed = markdown_to_roff(ch.content.as_str(), &arena);
            page = page.section(ch.name.as_str(), parsed);
            pages.push(page);
        }
    }

    pages
}
