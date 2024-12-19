use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Node {
    EOI,
    Block {
        block_type: String,
        params: BTreeMap<String, String>,
        children: Vec<Node>,
    },
    RawContent(String),
}
