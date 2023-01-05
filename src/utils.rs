use reqwest::Url;

pub const ARCHIVE_DIR: &str = "archivoor";
pub const BASE_DIR: &str = "collections";
pub const BASE_URL: &str = "http://localhost";

pub fn normalize_url(base_url: String) -> Box<dyn Fn(&String) -> Option<String>> {
    return Box::new(move |url| {
        let new_url = Url::parse(url.as_str());
        match new_url {
            Ok(new_url) => Some(new_url.to_string()),
            Err(_e) => {
                if url.starts_with('/') {
                    Some(format!("{}{}", base_url, url))
                } else {
                    None
                }
            }
        }
    });
}
