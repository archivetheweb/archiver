use reqwest::Url;

pub const ARCHIVE_DIR: &str = "archivoor";
pub const BASE_DIR: &str = "collections";
pub const BASE_URL: &str = "http://localhost";

pub fn normalize_url_map(base_url: String) -> Box<dyn Fn(&String) -> Option<Url>> {
    return Box::new(move |url| normalize_url(&base_url, url));
}

pub fn normalize_url(base_url: &str, url: &String) -> Option<Url> {
    let new_url = Url::parse(url.as_str());
    match new_url {
        Ok(mut new_url) => {
            // we remove the fragments (#)
            new_url.set_fragment(None);
            Some(new_url)
        }
        Err(_e) => {
            if url.starts_with('/') {
                let mut u = Url::parse(format!("{}{}", base_url, url).as_str()).unwrap();
                u.set_fragment(None);
                Some(u)
            } else {
                None
            }
        }
    }
}

pub fn extract_url(url: Url) -> String {
    url.as_str().split("record/").nth(1).unwrap().to_string()
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn remove_path_fragments() {
        let s = "https://example.com#hello".to_string();
        assert_eq!(
            normalize_url_map("https://example.com".to_string())(&s)
                .unwrap()
                .to_string(),
            "https://example.com/"
        );

        let s1 = "/hello#test".to_string();
        assert_eq!(
            normalize_url_map("https://example.com".to_string())(&s1)
                .unwrap()
                .to_string(),
            "https://example.com/hello"
        );
    }

    #[test]
    fn url() {
        let s = Url::parse("https://archivetheweb.com").unwrap();

        assert_eq!(s.to_string(), "https://archivetheweb.com/");
    }
}
