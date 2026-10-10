use crate::ast::{Block, Document};
use crate::error::MarkdownError;
use crate::front_matter::Injection;
use crate::parser::parse;

/// Reads an included file. Receives the identity of the including file and the
/// path written in the directive; returns the included file's identity (used
/// to detect cycles and to resolve its own includes) and its contents.
pub type Loader<'a> = dyn FnMut(&str, &str) -> Result<(String, String), String> + 'a;

/// How many files deep includes may nest.
const MAX_DEPTH: usize = 10;

impl Document {
    /// Replaces every `{{< include "..." >}}` with the content it names, and
    /// reads `codeinjection_head`/`codeinjection_foot` given as `{file: path}`
    /// (verbatim, not processed as markdown) into their text.
    ///
    /// `source` identifies this document to the loader, which resolves paths
    /// relative to it. Included markdown is spliced in as blocks (its own
    /// front matter is ignored); with `lang="..."` the file becomes a code
    /// block instead. A `.html`/`.htm` file becomes a single HTML block. Includes
    /// inside included markdown files are resolved too.
    pub fn resolve_includes(
        &mut self,
        source: &str,
        loader: &mut Loader,
    ) -> Result<(), MarkdownError> {
        let front_matter = &mut self.front_matter;
        for (key, injection) in [
            ("codeinjection_head", &mut front_matter.codeinjection_head),
            ("codeinjection_foot", &mut front_matter.codeinjection_foot),
        ] {
            if let Some(Injection::File(path)) = injection {
                let (_, content) = loader(source, path)
                    .map_err(|e| MarkdownError::Include(format!("{key}: {path}: {e}")))?;
                *injection = Some(Injection::Inline(content));
            }
        }

        let mut stack = vec![source.to_string()];
        self.body = resolve(std::mem::take(&mut self.body), &mut stack, loader)?;
        Ok(())
    }
}

fn is_html(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    path.ends_with(".html") || path.ends_with(".htm")
}

fn resolve(
    blocks: Vec<Block>,
    stack: &mut Vec<String>,
    loader: &mut Loader,
) -> Result<Vec<Block>, MarkdownError> {
    let mut out = Vec::with_capacity(blocks.len());
    for block in blocks {
        let Block::Include { path, lang } = block else {
            out.push(block);
            continue;
        };

        let from = stack.last().expect("stack always has the source").clone();
        let (id, content) =
            loader(&from, &path).map_err(|e| MarkdownError::Include(format!("{path}: {e}")))?;

        if let Some(start) = stack.iter().position(|s| *s == id) {
            let chain: Vec<&str> = stack[start..]
                .iter()
                .map(String::as_str)
                .chain([id.as_str()])
                .collect();
            return Err(MarkdownError::Include(format!(
                "{path}: includes form a cycle ({})",
                chain.join(" -> ")
            )));
        }
        if stack.len() > MAX_DEPTH {
            return Err(MarkdownError::Include(format!(
                "{path}: includes are nested more than {MAX_DEPTH} files deep"
            )));
        }

        match lang {
            Some(lang) => out.push(Block::CodeBlock {
                language: (!lang.is_empty()).then_some(lang),
                code: content.trim_end_matches(['\n', '\r']).to_string(),
            }),
            // An HTML file is one HTML card, verbatim. Markdown would end an
            // HTML block at its first blank line and parse what follows.
            None if is_html(&path) => out.push(Block::Html(
                content.trim_matches(['\n', '\r']).trim_end().to_string(),
            )),
            None => {
                let included = parse(&content)?.body;
                stack.push(id);
                let resolved = resolve(included, stack, loader);
                stack.pop();
                out.extend(resolved?);
            }
        }
    }
    Ok(out)
}
