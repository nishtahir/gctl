use crate::GhostError;

pub struct GhostAdminToken<'a> {
    pub id: &'a str,
    pub secret: &'a str,
}

impl<'a> GhostAdminToken<'a> {
    pub fn parse(value: &'a str) -> Result<Self, GhostError> {
        let components: Vec<&str> = value.split(":").collect::<Vec<&str>>();
        if components.len() != 2 {
            return Err(GhostError::ParseError(format!(
                "Invalid ghost admin token {}",
                value
            )));
        }

        Ok(GhostAdminToken {
            id: components[0],
            secret: components[1],
        })
    }
}
