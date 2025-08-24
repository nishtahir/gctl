use bon::Builder;
use url::Url;

use crate::GhostVersion;

#[derive(Builder)]
pub struct GhostContentApiOptions<'a> {
    pub url: Url,
    pub id: &'a str,
    pub secret: &'a str,
    pub version: GhostVersion,
    pub ghost_path: &'a str,
    pub user_agent: &'a str,
}
