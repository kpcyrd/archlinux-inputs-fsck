use crate::errors::*;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    pub r#ref: String,
    pub object: TagObject,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TagObject {
    pub sha: String,
    pub r#type: String,
    pub url: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagUrl {
    pub owner: String,
    pub name: String,
    pub tag: String,
}

pub fn detect_signed_tag_from_url(url: &str) -> Result<Option<TagUrl>> {
    let re = Regex::new(r"^https://github.com/([^/]+)/([^/]+)/archive/refs/tags/(.+).tar.gz$")?;
    if let Some(caps) = re.captures(url) {
        let owner = &caps[1];
        let name = &caps[2];
        let tag = &caps[3];

        return Ok(Some(TagUrl {
            owner: owner.to_string(),
            name: name.to_string(),
            tag: tag.to_string(),
        }));
    }

    let re = Regex::new(r"^https://github.com/([^/]+)/([^/]+)/archive/(.+)/.+\.tar\.gz$")?;
    if let Some(caps) = re.captures(url) {
        let owner = &caps[1];
        let name = &caps[2];
        let tag = &caps[3];
        // let _filename = &caps[4];

        return Ok(Some(TagUrl {
            owner: owner.to_string(),
            name: name.to_string(),
            tag: tag.to_string(),
        }));
    }

    let re = Regex::new(r"^https://github.com/([^/]+)/([^/]+)/archive/(.+)\.tar\.gz$")?;
    if let Some(caps) = re.captures(url) {
        let owner = &caps[1];
        let name = &caps[2];
        let tag = &caps[3];

        return Ok(Some(TagUrl {
            owner: owner.to_string(),
            name: name.to_string(),
            tag: tag.to_string(),
        }));
    }

    Ok(None)
}

pub async fn fetch_tag(client: &Client, owner: &str, name: &str, tag: &str) -> Result<Tag> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/git/ref/tags/{}",
        owner, name, tag
    );

    info!("Url={}", url);
    let json = client
        .get(url)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_url_matching() -> Result<()> {
        let x = detect_signed_tag_from_url(
            "https://github.com/kpcyrd/acme-redirect/archive/v0.5.3/acme-redirect-0.5.3.tar.gz",
        )?;
        assert_eq!(
            x,
            Some(TagUrl {
                owner: "kpcyrd".to_string(),
                name: "acme-redirect".to_string(),
                tag: "v0.5.3".to_string(),
            })
        );

        let x = detect_signed_tag_from_url(
            "https://github.com/abseil/abseil-cpp/archive/20211102.0/abseil-cpp-20211102.0.tar.gz",
        )?;
        assert_eq!(
            x,
            Some(TagUrl {
                owner: "abseil".to_string(),
                name: "abseil-cpp".to_string(),
                tag: "20211102.0".to_string(),
            })
        );

        Ok(())
    }
}
