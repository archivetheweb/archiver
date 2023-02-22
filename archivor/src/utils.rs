use rand::{distributions::Alphanumeric, thread_rng, Rng};
use reqwest::Url;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const ARCHIVE_DIR: &str = "archivoor";
pub const BASE_URL: &str = "http://localhost";
pub const WARC_APPLICATION_TYPE: &str = "application/warc";

pub const CONTRACT_ADDRESS: &str = "-27RfG2DJAI3ddQlrXkN1rmS5fBSC4eG8Zfhz8skYTU";

pub fn normalize_url_map(base_url: String) -> Box<dyn Fn(&String) -> Option<String>> {
    return Box::new(move |url| normalize_url(&base_url, url));
}

pub fn normalize_url(base_url: &str, url: &String) -> Option<String> {
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
}

pub fn jitter(duration: Duration) -> Duration {
    let mut rng = rand::thread_rng();
    let rndm = rng.gen_range(1.0..10.0);
    duration.mul_f64(rndm)
}

pub fn extract_url(url: &str) -> String {
    url.split("record/").nth(1).unwrap().to_string()
}

pub fn extract_collection_name(url: &str) -> String {
    url.split("/").nth(3).unwrap().to_string()
}

pub fn get_unix_timestamp() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
}

pub fn get_tmp_screenshot_dir(collection_name: &str) -> String {
    format!("/tmp/archivoor_{}.png", collection_name)
}

pub fn get_random_string(len: i32) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len as usize)
        .map(char::from)
        .collect()
}

pub fn create_random_tmp_folder() -> anyhow::Result<PathBuf> {
    let rand_folder_name: String = get_random_string(11);

    let path = PathBuf::from(format!("/tmp/archivoor-{}", rand_folder_name));
    fs::create_dir(&path)?;
    // populate this folder with a config.yaml
    Ok(path)
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

    #[test]
    fn extract_collection_name_test() {
        let s = extract_collection_name(
            "http://localhost:8272/A5U3DMjDdMz/record/https://example.com.png".into(),
        );
        assert_eq!(s, "A5U3DMjDdMz");
    }

    #[test]
    fn creates_a_random_folder() {
        // let path = ""
        let p = create_random_tmp_folder().unwrap();
        assert!(p.exists());
        fs::remove_dir(p).unwrap();
    }
}
