use crate::ast::Node;
use crate::error::GhostLiquidError;
use crate::visitor::GhostVisitor;
use pest::{
    iterators::{Pair, Pairs},
    Parser,
};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(pest_derive::Parser)]
#[grammar = "liquid.pest"]
pub struct LiquidParser {}

impl LiquidParser {
    pub fn parse_template(input: &str) -> Result<Value, GhostLiquidError> {
        let pairs = LiquidParser::parse(Rule::root, input)?;
        let nodes = parse_nodes(pairs);

        let mut visitor = GhostVisitor::new();
        nodes[0].accept(&mut visitor)
    }
}

fn parse_nodes(pairs: Pairs<Rule>) -> Vec<Node> {
    let mut nodes = Vec::new();
    let next = pairs.into_iter().next();
    if let Some(next) = next {
        nodes.push(parse_node(next));
    }
    nodes
}

fn parse_node(pair: Pair<Rule>) -> Node {
    match pair.as_rule() {
        Rule::block => {
            let mut inner = pair.into_inner();
            let block_type = inner.next().unwrap().as_str().to_string();

            let block_params = inner.next().unwrap().into_inner();
            let mut params = BTreeMap::new();

            for param in block_params {
                if param.as_rule() == Rule::block_param {
                    let mut param_inner = param.into_inner();
                    let key = param_inner.next().unwrap().as_str().to_string();
                    let value = param_inner
                        .next()
                        .unwrap()
                        .as_str()
                        .trim_matches('"')
                        .to_string();
                    params.insert(key, value);
                }
            }

            let block_children = inner.next().unwrap();
            let mut children = vec![];
            if block_children.as_rule() == Rule::block_children {
                for item in parse_nodes(block_children.into_inner()) {
                    children.push(item);
                }
            }

            Node::Block {
                block_type,
                params,
                children,
            }
        }
        Rule::raw_content => Node::RawContent(pair.as_str().to_string()),
        Rule::EOI => Node::EOI,
        rule => unreachable!("{:?}", rule),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_template() {
        let output = LiquidParser::parse_template("").unwrap();
        insta::assert_json_snapshot!(output);
    }

    #[test]
    fn test_post_block_template() {
        let content = r#"{% post id: "foo"
        title: "bar"
        slug: "baz"
        updated: "qux"
        %} {% endpost %}"#;

        let output = LiquidParser::parse_template(content).unwrap();
        insta::assert_json_snapshot!(output);
    }

    #[test]
    fn test_nested_post_block_template() {
        let content = r#"{% post id: "foo"
        title: "bar"
        slug: "baz"
        updated: "qux"
        %}
        {% markdown %}
        Some content
        {% endmarkdown %}
        {% endpost %}"#;

        let output = LiquidParser::parse_template(content).unwrap();
        insta::assert_json_snapshot!(output);
    }
}
