use url::Url;

#[derive(Debug)]
pub enum GhostVersion {
    V3,
    V4,
    V5,
}

impl AsRef<str> for GhostVersion {
    fn as_ref(&self) -> &str {
        match self {
            GhostVersion::V3 => "v3.0",
            GhostVersion::V4 => "v4.0",
            GhostVersion::V5 => "v5.0",
        }
    }
}

pub struct GhostContentApiOptions {
    pub url: Url,
    pub id: String,
    pub secret: String,
    pub version: GhostVersion,
    pub ghost_path: String,
    pub user_agent: String,
}
