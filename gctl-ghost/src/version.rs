use crate::GhostError;

#[derive(Debug)]
pub enum GhostVersion {
    V3,
    V4,
    V5,
    V6,
}

impl AsRef<str> for GhostVersion {
    fn as_ref(&self) -> &str {
        match self {
            GhostVersion::V3 => "v3.0",
            GhostVersion::V4 => "v4.0",
            GhostVersion::V5 => "v5.0",
            GhostVersion::V6 => "v6.0",
        }
    }
}

impl TryFrom<&str> for GhostVersion {
    type Error = GhostError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "v3" | "v3.0" => Ok(GhostVersion::V3),
            "v4" | "v4.0" => Ok(GhostVersion::V4),
            "v5" | "v5.0" => Ok(GhostVersion::V5),
            "v6" | "v6.0" => Ok(GhostVersion::V6),
            _ => Err(GhostError::ParseError(format!(
                "Invalid Ghost Api Version {}",
                value
            ))),
        }
    }
}
