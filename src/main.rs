// src/main.rs
extern crate mdbook;

use mdbook::renderer::RenderContext;
use serde::{Deserialize, Serialize};

use std::{fs, io, path::PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct ManOutputConfiguration {
    /// If specified the pages will be saved as files rather than printed to stdout.
    pub output_dir: Option<PathBuf>,
    #[serde(default)]
    /// Wether to split the book into separate files per chapter or render one man page with all chapters.
    pub split_chapters: bool,
    /// Override the name of the output file if `output_dir` is also specified.
    pub filename: Option<String>,
}

impl ManOutputConfiguration {
    fn load(ctx: &RenderContext) -> Self {
        ctx.config
            .get_deserialized_opt("output.man")
            .ok()
            .flatten()
            .unwrap_or_default()
    }
}

fn main() {
    let mut stdin = io::stdin();
    let ctx = RenderContext::from_json(&mut stdin).unwrap();
    let cfg = ManOutputConfiguration::load(&ctx);

    if !cfg.split_chapters {
        let page = mdbook_man::mdbook_to_roff(&ctx);

        let page = page.to_string().unwrap();

        if let Some(path) = cfg.output_dir {
            if !path.exists() {
                fs::create_dir_all(&path).unwrap();
            }
            let filename = if let Some(filename) = &cfg.filename {
                filename
            } else {
                "book.man"
            };
            fs::write(path.join(filename), page).unwrap()
        } else {
            println!("{}", page)
        }
    } else {
        let pages = mdbook_man::mdbook_to_roff_chapters(&ctx);

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
