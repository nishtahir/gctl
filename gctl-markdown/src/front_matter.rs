use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Deserializer};

use crate::error::MarkdownError;

/// Front matter understood by the publisher. Field names follow Hugo
/// conventions; unknown keys are ignored.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct FrontMatter {
    /// Ghost post id. When present the post is updated by id rather than slug.
    pub id: Option<String>,
    pub title: Option<String>,
    pub slug: Option<String>,
    #[serde(alias = "publishDate", alias = "publishdate")]
    pub date: Option<String>,
    /// `draft`, `published` or `scheduled`. Takes precedence over `draft`.
    pub status: Option<String>,
    pub draft: Option<bool>,
    #[serde(deserialize_with = "one_or_many")]
    pub tags: Vec<String>,
    /// Hugo taxonomy; merged into tags on publish.
    #[serde(deserialize_with = "one_or_many")]
    pub categories: Vec<String>,
    #[serde(alias = "summary")]
    pub description: Option<String>,
    #[serde(alias = "image", alias = "cover")]
    pub feature_image: Option<String>,
    pub featured: Option<bool>,
    pub visibility: Option<String>,
    #[serde(alias = "canonical")]
    pub canonical_url: Option<String>,
}

fn one_or_many<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    Ok(match Option::<OneOrMany>::deserialize(d)? {
        None => vec![],
        Some(OneOrMany::One(one)) => vec![one],
        Some(OneOrMany::Many(many)) => many,
    })
}

impl FrontMatter {
    /// Tags and categories, without duplicates, in declaration order.
    pub fn all_tags(&self) -> Vec<String> {
        let mut all: Vec<String> = Vec::new();
        for tag in self.tags.iter().chain(&self.categories) {
            if !all.contains(tag) {
                all.push(tag.clone());
            }
        }
        all
    }

    /// The `date` as a UTC timestamp. Dates without a time are midnight UTC.
    pub fn published_at(&self) -> Result<Option<DateTime<Utc>>, MarkdownError> {
        self.date.as_deref().map(parse_date).transpose()
    }
}

fn parse_date(input: &str) -> Result<DateTime<Utc>, MarkdownError> {
    let input = input.trim();
    if let Ok(date) = DateTime::parse_from_rfc3339(input) {
        return Ok(date.with_timezone(&Utc));
    }
    for format in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"] {
        if let Ok(date) = NaiveDateTime::parse_from_str(input, format) {
            return Ok(date.and_utc());
        }
    }
    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        return Ok(date.and_hms_opt(0, 0, 0).unwrap().and_utc());
    }
    Err(MarkdownError::Date(input.to_string()))
}
