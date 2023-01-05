use reqwest::Url;

pub const ARCHIVE_DIR: &str = "archivoor";
pub const BASE_DIR: &str = "collections";
pub const BASE_URL: &str = "http://localhost";

pub fn normalize_url(base_url: String) -> Box<dyn Fn(&String) -> Option<String>> {
    return Box::new(move |url| {
        let new_url = Url::parse(url.as_str());
        match new_url {
            Ok(mut new_url) => {
                // we remove the fragments (#)
                new_url.set_fragment(None);
                Some(new_url.to_string())
            }
            Err(_e) => {
                if url.starts_with('/') {
                    let mut u = Url::parse(format!("{}{}", base_url, url).as_str()).unwrap();
                    u.set_fragment(None);
                    Some(u.to_string())
                } else {
                    None
                }
            }
        }
    });
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn remove_path_fragments() {
        let s = "https://example.com#hello".to_string();
        assert_eq!(
            normalize_url("https://example.com".to_string())(&s).unwrap(),
            "https://example.com/"
        );

        let s1 = "/hello#test".to_string();
        assert_eq!(
            normalize_url("https://example.com".to_string())(&s1).unwrap(),
            "https://example.com/hello"
        );
    }
}
