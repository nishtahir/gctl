use reqwest::Client;
use url::Url;

use crate::images::GhostImageApi;
use crate::options::GhostContentApiOptions;
use crate::posts::GhostPostsApi;
use crate::GhostAdminToken;
use crate::GhostError;
use crate::GhostVersion;

pub struct GhostContentApi<'a> {
    options: GhostContentApiOptions<'a>,
    client: Client,
}

impl<'a> GhostContentApi<'a> {
    pub fn new(
        url: Url,
        admin_token: GhostAdminToken,
        version: GhostVersion,
    ) -> Result<GhostContentApi, GhostError> {
        let options = GhostContentApiOptions::builder()
            .url(url)
            .id(admin_token.id)
            .secret(admin_token.secret)
            .version(version)
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:64.0) Gecko/20100101 Firefox/64.0",
            )
            .ghost_path("ghost")
            .build();

        let client = Client::builder().build()?;
        Ok(GhostContentApi { options, client })
    }

    pub fn posts(&self) -> GhostPostsApi {
        GhostPostsApi::new(&self.options, &self.client)
    }

    pub fn images(&self) -> GhostImageApi {
        GhostImageApi::new(&self.options, &self.client)
    }
}
