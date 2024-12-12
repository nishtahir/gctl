use url::Url;

struct TrailingSlashUrlBulder {
    url: Url,
}

impl TrailingSlashUrlBulder {
    fn new(url: Url) -> TrailingSlashUrlBulder {
        TrailingSlashUrlBulder { url }
    }

    fn append_path(&mut self, path: &str) -> &mut Self {
        let mut url = self.url.clone();

        // pop until we find a non-empty segment
        url.path_segments_mut()
            .unwrap()
            .pop_if_empty()
            .push(path)
            .push("");
        self.url = url;
        self
    }

    fn as_str(&self) -> &str {
        self.url.as_str()
    }

    fn build(&self) -> Url {
        self.url.clone()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_url_builder() {
        let url = Url::parse("http://localhost:2368").unwrap();
        let builder = TrailingSlashUrlBulder::new(url);
        assert_eq!(builder.as_str(), "http://localhost:2368/");
    }

    #[test]
    fn test_url_builder_append_path() {
        let url = Url::parse("http://localhost:2368").unwrap();
        let mut builder = TrailingSlashUrlBulder::new(url);
        builder.append_path("test");
        assert_eq!(builder.as_str(), "http://localhost:2368/test/");
    }
}
