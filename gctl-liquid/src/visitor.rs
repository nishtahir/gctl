use std::collections::BTreeMap;

use crate::ast::Node;
use crate::error::GhostLiquidError;
use serde_json::Value;

pub trait Visitor: Sized {
    type Output;
    type Error;

    fn visit_block(
        &mut self,
        block_type: &str,
        params: &BTreeMap<String, String>,
        children: &[Node],
    ) -> Result<Self::Output, Self::Error>;
    fn visit_raw_content(&mut self, content: &str) -> Result<Self::Output, Self::Error>;
    fn visit_eoi(&mut self) -> Result<Self::Output, Self::Error>;
}

impl Node {
    pub fn accept<V: Visitor>(&self, visitor: &mut V) -> Result<V::Output, V::Error> {
        match self {
            Node::Block {
                block_type,
                params,
                children,
            } => visitor.visit_block(block_type, params, children),
            Node::RawContent(content) => visitor.visit_raw_content(content),
            Node::EOI => visitor.visit_eoi(),
        }
    }
}

pub struct GhostVisitor {}

impl GhostVisitor {
    pub fn new() -> Self {
        Self {}
    }

    fn visit_post(
        &mut self,
        params: &BTreeMap<String, String>,
        children: &[Node],
    ) -> Result<Value, GhostLiquidError> {
        let children = children
            .iter()
            .map(|child| child.accept(self))
            .collect::<Result<Vec<Value>, GhostLiquidError>>()?;

        Ok(serde_json::json!({
            "id": params.get("id"),
            "title": params.get("title"),
            "slug": params.get("slug"),
            "updated": params.get("updated"),
            "lexical": serde_json::json!({
                "root": {
                    "children": children,
                    "direction": "ltr",
                    "format": "",
                    "indent": 0,
                    "type": "root",
                    "version": 1
                }
            }).to_string(),
        }))
    }

    fn visit_markdown(
        &mut self,
        _: &BTreeMap<String, String>,
        children: &[Node],
    ) -> Result<Value, GhostLiquidError> {
        let children = children
            .iter()
            .map(|child| child.accept(self))
            .collect::<Result<Vec<Value>, GhostLiquidError>>()?;

        let markdown = children.iter()
            .map(|v| v.as_str().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("");

        Ok(serde_json::json!({
            "type": "markdown",
            "version": 1,
            "markdown": markdown,
        }))
    }
}

impl Visitor for GhostVisitor {
    type Output = Value;
    type Error = GhostLiquidError;

    fn visit_block(
        &mut self,
        block_type: &str,
        params: &BTreeMap<String, String>,
        children: &[Node],
    ) -> Result<Self::Output, Self::Error> {
        // visit by block type and visit children
        match block_type {
            "post" => self.visit_post(params, children),
            "markdown" => self.visit_markdown(params, children),
            _ => unimplemented!("Unknown block type: {}", block_type),
        }
    }

    fn visit_raw_content(&mut self, content: &str) -> Result<Self::Output, Self::Error> {
        Ok(Value::String(content.to_owned()))
    }

    fn visit_eoi(&mut self) -> Result<Self::Output, Self::Error> {
        Ok(Value::String("".to_owned()))
    }
}
