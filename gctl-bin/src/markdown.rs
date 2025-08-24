use gctl_ghost::Post;

use crate::error::GctlCliError;
use gctl_liquid::LiquidParser;

pub fn parse_jekyll_formated_markdown(content: &str) -> Result<Post, GctlCliError> {
    if !content.trim_start().starts_with("---") {
        return Err(GctlCliError::ParseError("".into()));
    }
    let mut parts = content.splitn(3, "---");
    parts.next();

    let yaml_str = parts.next().unwrap();
    let content_str = parts.next().unwrap();
    let front_matter: serde_yaml::Value = serde_yaml::from_str(yaml_str).unwrap();

    let mut liquid_template = String::new();
    liquid_template += "{% post\n";
    for (key, value) in front_matter.as_mapping().unwrap().into_iter() {
        liquid_template += &format!(
            "{}: \"{}\"\n",
            serde_yaml::to_string(&key).unwrap().trim(),
            serde_yaml::to_string(&value).unwrap().trim()
        );
    }
    liquid_template += "%}\n";
    liquid_template += "{% markdown %}\n";
    liquid_template += content_str.trim();
    liquid_template += "\n";
    liquid_template += "{% endmarkdown %}\n";
    liquid_template += "{% endpost %}\n";

    let parsed = LiquidParser::parse_template(&liquid_template).unwrap();
    Ok(serde_json::from_value(parsed).unwrap())
}

#[cfg(test)]
mod test {
    use insta::assert_json_snapshot;

    use super::*;

    #[test]
    fn test_markdown_parsing() {
        let content = r#"---
        slug: hello-world
        title: Hello World
        id: foo
        ---

        This is some content in markdown
        "#;

        let actual = parse_jekyll_formated_markdown(content).unwrap();
        assert_json_snapshot!(actual);
    }
}
