use console::style;
use gctl_ghost::{GhostAdminToken, GhostContentApi, GhostVersion, PostFilter};
use url::Url;

use crate::error::GctlCliError;

#[derive(Debug, PartialEq, Eq)]
enum Level {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, PartialEq, Eq)]
struct Check {
    level: Level,
    message: String,
}

fn check(level: Level, message: impl Into<String>) -> Check {
    Check {
        level,
        message: message.into(),
    }
}

fn is_hex(value: &str, len: usize) -> bool {
    value.len() == len && value.chars().all(|c| c.is_ascii_hexdigit())
}

/// What is wrong with the site address, if anything.
fn url_checks(raw: &str) -> (Vec<Check>, Option<Url>) {
    let url = match Url::parse(raw) {
        Ok(url) if matches!(url.scheme(), "http" | "https") && url.has_host() => url,
        Ok(_) | Err(_) => {
            return (
                vec![check(
                    Level::Fail,
                    format!("`{raw}` is not a web address; use the form https://yourblog.ghost.io"),
                )],
                None,
            )
        }
    };
    let mut checks = vec![check(Level::Ok, format!("Site address: {url}"))];
    if url.path().trim_end_matches('/').ends_with("/ghost") {
        checks.push(check(
            Level::Warn,
            "the address ends in /ghost; use the site's address, without the admin path",
        ));
    }
    if url.scheme() == "http" && !matches!(url.host_str(), Some("localhost" | "127.0.0.1")) {
        checks.push(check(
            Level::Warn,
            "the address uses http, so the API key's token travels unencrypted",
        ));
    }
    (checks, Some(url))
}

/// What is wrong with the Admin API key, if anything.
fn key_checks(raw: &str) -> Vec<Check> {
    if GhostAdminToken::parse(raw).is_err() {
        let hint = if is_hex(raw, 26) {
            " (it looks like a Content API key; gctl needs the Admin API key)"
        } else {
            ""
        };
        return vec![check(
            Level::Fail,
            format!("The Admin API key should look like `id:secret`{hint}"),
        )];
    }
    let (id, secret) = raw.split_once(':').unwrap_or_default();
    if is_hex(id, 24) && is_hex(secret, 64) {
        vec![check(Level::Ok, format!("Admin API key id: {id}"))]
    } else {
        vec![check(
            Level::Fail,
            "The Admin API key should be a 24 character id and a 64 character secret, \
separated by `:`; copy it again from Settings > Integrations",
        )]
    }
}

/// A suggestion for an error from Ghost.
fn hint_for(error: &str) -> &'static str {
    if error.contains("401") || error.contains("403") {
        "Ghost rejected the key. Check that it is the Admin API key of a custom integration on this site"
    } else if error.contains("404") {
        "Ghost has no Admin API at this address. Check the site address and --ghost-api-version"
    } else if error.contains("HTTP error") {
        "Could not reach the site. Check the address and your connection"
    } else {
        "See the message above"
    }
}

pub async fn run(
    url: Option<String>,
    key: Option<String>,
    version: &str,
    from_stored: bool,
) -> Result<(), GctlCliError> {
    let source = if from_stored {
        "stored login"
    } else {
        "flags or environment"
    };
    let mut checks = Vec::new();
    let mut parsed_url = None;
    let mut key_ok = false;

    match &url {
        Some(url) => {
            let (found, parsed) = url_checks(url);
            checks.push(check(Level::Ok, format!("Credentials from {source}")));
            checks.extend(found);
            parsed_url = parsed;
        }
        None => checks.push(check(
            Level::Fail,
            "No Ghost URL configured. Run `gctl auth login`",
        )),
    }
    match &key {
        Some(key) => {
            let found = key_checks(key);
            key_ok = found.iter().all(|c| c.level != Level::Fail);
            checks.extend(found);
        }
        None => checks.push(check(
            Level::Fail,
            "No Admin API key configured. Run `gctl auth login`",
        )),
    }

    match GhostVersion::try_from(version) {
        Ok(_) => checks.push(check(Level::Ok, format!("API version: {version}"))),
        Err(e) => checks.push(check(Level::Fail, format!("API version `{version}`: {e}"))),
    }

    if let (Some(url), Some(key), true) = (parsed_url, key.as_deref(), key_ok) {
        if let (Ok(token), Ok(api_version)) =
            (GhostAdminToken::parse(key), GhostVersion::try_from(version))
        {
            let api = GhostContentApi::new(url, token, api_version)?;
            let filter = PostFilter {
                limit: Some(1),
                ..PostFilter::default()
            };
            match api.posts().browse(&filter).await {
                Ok(_) => checks.push(check(Level::Ok, "Connected, and the key can read posts")),
                Err(e) => {
                    let message = e.to_string();
                    checks.push(check(
                        Level::Fail,
                        format!("{message}\n    {}", hint_for(&message)),
                    ));
                }
            }
        }
    }

    let mut failures = 0;
    for Check { level, message } in &checks {
        let mark = match level {
            Level::Ok => style("ok").green(),
            Level::Warn => style("warn").yellow(),
            Level::Fail => {
                failures += 1;
                style("FAIL").red()
            }
        };
        println!("[{mark}] {message}");
    }
    if failures > 0 {
        return Err(GctlCliError::Config(format!("{failures} check(s) failed")));
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    fn summary(checks: Vec<Check>) -> Vec<String> {
        checks
            .into_iter()
            .map(|c| format!("{:?}: {}", c.level, c.message))
            .collect()
    }

    #[test]
    fn addresses() {
        let results: Vec<_> = [
            "https://blog.example.com",
            "https://blog.example.com/ghost/",
            "http://blog.example.com",
            "http://localhost:2368",
            "blog.example.com",
            "ftp://blog.example.com",
        ]
        .iter()
        .map(|raw| (*raw, summary(url_checks(raw).0)))
        .collect();
        insta::assert_debug_snapshot!(results);
    }

    #[test]
    fn keys() {
        let id = "6891829ae317750001061429";
        let secret = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let results: Vec<_> = [
            format!("{id}:{secret}"),
            "0123456789abcdef0123456789".to_string(),
            "abc:def".to_string(),
            format!("{id}:{secret}:extra"),
        ]
        .iter()
        .map(|raw| (raw.replace(secret, "<secret>"), summary(key_checks(raw))))
        .collect();
        insta::assert_debug_snapshot!(results);
    }

    #[test]
    fn hints() {
        let hints: Vec<_> = [
            "API error: 401 Unauthorized - ...",
            "API error: 404 Not Found",
            "HTTP error: error sending request",
            "weird",
        ]
        .iter()
        .map(|e| (*e, hint_for(e)))
        .collect();
        insta::assert_debug_snapshot!(hints);
    }
}
