use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use std::fmt;

use serde::de::{self, MapAccess, Visitor};
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
    /// `post` (the default) or `page`.
    #[serde(rename = "type")]
    pub kind: Option<String>,
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
    /// HTML injected into the page's `<head>` (Ghost "code injection"), as text
    /// or `{file: path}`. An empty string clears what the post has; leaving
    /// the key out keeps it.
    pub codeinjection_head: Option<Injection>,
    /// HTML injected before the closing `</body>`.
    pub codeinjection_foot: Option<Injection>,
    /// Content kinds `gctl pull` had to approximate in markdown (e.g.
    /// `callout`, `footnote`). Pushing such a file would change the post.
    #[serde(deserialize_with = "one_or_many")]
    pub lossy: Vec<String>,
}

/// Code to inject into a page: written inline, or read from a file.
#[derive(Debug, Clone, PartialEq)]
pub enum Injection {
    Inline(String),
    /// `{file: path}`, replaced by [`Inline`](Injection::Inline) when
    /// includes are resolved.
    File(String),
}

impl<'de> Deserialize<'de> for Injection {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct InjectionVisitor;

        impl<'de> Visitor<'de> for InjectionVisitor {
            type Value = Injection;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("HTML text or a `{file: path}` mapping")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Injection, E> {
                Ok(Injection::Inline(value.to_string()))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Injection, A::Error> {
                let mut file = None;
                while let Some(key) = map.next_key::<String>()? {
                    if key == "file" {
                        file = Some(map.next_value::<String>()?);
                    } else {
                        return Err(de::Error::unknown_field(&key, &["file"]));
                    }
                }
                file.map(Injection::File)
                    .ok_or_else(|| de::Error::missing_field("file"))
            }
        }

        deserializer.deserialize_any(InjectionVisitor)
    }
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

    /// The head injection as text. An error if it still refers to a file
    /// because includes weren't resolved: dropping it silently would be worse.
    pub fn head_injection(&self) -> Result<Option<&str>, MarkdownError> {
        injection_text("codeinjection_head", &self.codeinjection_head)
    }

    /// The foot injection as text; see [`head_injection`](Self::head_injection).
    pub fn foot_injection(&self) -> Result<Option<&str>, MarkdownError> {
        injection_text("codeinjection_foot", &self.codeinjection_foot)
    }

    /// The `date` as a UTC timestamp. Dates without a time are midnight UTC.
    pub fn published_at(&self) -> Result<Option<DateTime<Utc>>, MarkdownError> {
        self.date.as_deref().map(parse_date).transpose()
    }
}

fn injection_text<'a>(
    key: &str,
    injection: &'a Option<Injection>,
) -> Result<Option<&'a str>, MarkdownError> {
    match injection {
        None => Ok(None),
        Some(Injection::Inline(text)) => Ok(Some(text)),
        Some(Injection::File(path)) => Err(MarkdownError::Include(format!(
            "{key}: {path}: file reference was not resolved"
        ))),
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
