use anyhow::Context;
use futures::Stream;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use reqwest::Url;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::types::UrlInfo;

pub const ARCHIVE_DIR: &str = "archiver";
pub const BASE_URL: &str = "http://localhost";
pub const WARC_APPLICATION_TYPE: &str = "application/warc";
pub const FORMAT_STRING: &str = "%Y%m%d%H%M%S";
pub const BUNDLR_URL: &str = "https://node1.bundlr.network";
pub const CHUNKING_THRESHOLD: usize = 50_000_000;

lazy_static! {
    pub static ref CONTRACT_ADDRESS: String = {
        let env = match std::env::var("ENVIRONMENT") {
            Ok(e) => e,
            Err(_) => "".into(),
        };
        if env == "production" {
            return "dD1DuvgM_Vigtnv4vl2H1IYn9CgLvYuhbEWPOL-_4Mw".into();
        }
        "-27RfG2DJAI3ddQlrXkN1rmS5fBSC4eG8Zfhz8skYTU".into()
    };
    pub static ref APP_NAME: String = {
        let env = match std::env::var("ENVIRONMENT") {
            Ok(e) => e,
            Err(_) => "".into(),
        };
        if env == "production" {
            return "archivetheweb".into();
        }
        "atw".into()
    };
    pub static ref APP_VERSION: String = {
        let env = match std::env::var("ENVIRONMENT") {
            Ok(e) => e,
            Err(_) => "".into(),
        };
        let v = "0.0.1";
        if env == "production" {
            return v.into();
        }
        format!("{}_dev", v)
    };
}

pub fn normalize_url_map(base_url: String) -> Box<dyn Fn(&String) -> Option<UrlInfo>> {
    return Box::new(move |url| normalize_url(&base_url, url));
}

pub fn normalize_url(base_url: &str, url: &String) -> Option<UrlInfo> {
    let new_url = Url::parse(url.as_str());
    match new_url {
        // https://localhost:<PORT>/<ARCHIVE_NAME>/record/<URL>
        Ok(mut new_url) => {
            let scheme = new_url.scheme();
            if scheme != "https" && scheme != "http" {
                return None;
            }

            // we remove the fragments (#)
            new_url.set_fragment(None);

            let domain = match get_domain(&extract_url(new_url.as_str())) {
                Ok(d) => d,
                Err(e) => {
                    debug!("URL: {}, could not get domain {}", new_url.as_str(), e);
                    return None;
                }
            };

            Some(UrlInfo {
                url: standardize_url(new_url.as_str()),
                domain: domain,
            })
        }
        Err(_e) => {
            if url.starts_with('/') {
                let mut u = match Url::parse(format!("{}{}", base_url, url).as_str()) {
                    Ok(u) => u,
                    Err(_) => return None,
                };

                let domain = match get_domain(&extract_url(u.as_str())) {
                    Ok(d) => d,
                    Err(e) => {
                        debug!("pError, URL: {}, could not get domain {}", u.as_str(), e);
                        return None;
                    }
                };

                u.set_fragment(None);
                Some(UrlInfo {
                    url: standardize_url(u.as_str()),
                    domain: domain,
                })
            } else {
                return None;
            }
        }
    }
}

pub fn get_domain(url: &str) -> anyhow::Result<String> {
    let u = Url::parse(url)?;
    let u = u.domain();
    match u {
        Some(u) => Ok(u.replace("www.", "")),
        None => return Err(anyhow::anyhow!("could not get domain")),
    }
}

pub fn assert_stream_send<'u, R>(
    strm: impl 'u + Send + Stream<Item = R>,
) -> impl 'u + Send + Stream<Item = R> {
    strm
}

pub fn jitter(duration: Duration) -> Duration {
    let mut rng = rand::thread_rng();
    let rndm = rng.gen_range(1.0..10.0);
    duration.mul_f64(rndm)
}

pub fn extract_url(url: &str) -> String {
    if url.contains("/mp_/") {
        return url.split("record/mp_/").nth(1).unwrap().to_string();
    }
    url.split("record/").nth(1).unwrap().to_string()
}

fn standardize_url(url: &str) -> String {
    url.replace("/mp_/", "/").into()
}

pub fn extract_collection_name(url: &str) -> String {
    url.split("/").nth(3).unwrap().to_string()
}

pub fn get_unix_timestamp() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
}

pub fn get_tmp_screenshot_dir(collection_name: &str) -> String {
    format!("/tmp/archiver_{}.png", collection_name)
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

    let path = PathBuf::from(format!("/tmp/archiver-{}", rand_folder_name));
    fs::create_dir(&path).context("failed to create random_tmp_folder")?;
    Ok(path)
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_normalize() {
        struct Test {
            expected: Option<UrlInfo>,
            value: String,
        }
        let tests = vec![
            Test {
                value: "https://localhost:8080/aaaa/record/https://example.com#hello".into(),
                expected: Some(UrlInfo {
                    url: "https://localhost:8080/aaaa/record/https://example.com".into(),
                    domain: "example.com".into(),
                }),
            },
            Test {
                value: "https://localhost:8080/aaaa/record/https://www.example.com#hello".into(),
                expected: Some(UrlInfo {
                    url: "https://localhost:8080/aaaa/record/https://www.example.com".into(),
                    domain: "example.com".into(),
                }),
            },
            Test {
                value: "https://localhost:8080/aaaa/record/http://example.com".into(),
                expected: Some(UrlInfo {
                    url: "https://localhost:8080/aaaa/record/http://example.com".into(),
                    domain: "example.com".into(),
                }),
            },
            Test {
                value: "/aaaa/record/https://example.com/hello#test".into(),
                expected: Some(UrlInfo {
                    url: "https://localhost:8080/aaaa/record/https://example.com/hello".into(),
                    domain: "example.com".into(),
                }),
            },
            Test {
                value: "javascript:print();".into(),
                expected: None,
            },
            Test {
                value: "mailto:ex@ex.org".into(),
                expected: None,
            },
            Test {
                value: "fb-messenger://share?link=https%3A%2F%2Fwww.theguardian.com".into(),
                expected: None,
            },
        ];

        let n = normalize_url_map("https://localhost:8080".to_string());

        for test in tests {
            assert_eq!(n(&test.value), test.expected);
        }
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
        let p = create_random_tmp_folder().unwrap();
        assert!(p.exists());
        fs::remove_dir(p).unwrap();
    }
}
