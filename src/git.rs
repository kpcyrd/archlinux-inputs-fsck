use crate::errors::*;
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct GitSource {
    url: String,
    commit: Option<String>,
    tag: Option<String>,
    signed: bool,
}

impl GitSource {
    pub fn is_commit_securely_pinned(&self) -> bool {
        if let Some(commit) = &self.commit {
            is_git_object_hash(commit)
        } else if let Some(tag) = &self.tag {
            is_git_object_hash(tag)
        } else {
            false
        }
    }
}

fn is_git_object_hash(name: &str) -> bool {
    name.len() == 40 && name.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

impl FromStr for GitSource {
    type Err = Error;

    fn from_str(mut s: &str) -> Result<Self> {
        let mut signed = false;
        let mut commit = None;
        let mut tag = None;

        if let Some(remaining) = s.strip_suffix("?signed") {
            signed = true;
            s = remaining;
        }

        if let Some((remaining, value)) = s.rsplit_once("#commit=") {
            commit = Some(value.to_string());
            s = remaining;
        }

        if let Some((remaining, value)) = s.rsplit_once("#tag=") {
            tag = Some(value.to_string());
            s = remaining;
        }

        if let Some(remaining) = s.strip_suffix("?signed") {
            signed = true;
            s = remaining;
        }

        Ok(Self {
            url: s.to_string(),
            commit,
            tag,
            signed,
        })
    }
}
