use crate::errors::*;
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq)]
pub struct HgSource {
    url: String,
    revision: Option<String>,
}

impl HgSource {
    pub fn is_revision_securely_pinned(&self) -> bool {
        if let Some(revision) = &self.revision {
            is_hg_object_hash(revision)
        } else {
            false
        }
    }
}

fn is_hg_object_hash(name: &str) -> bool {
    name.len() == 40 && name.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

impl FromStr for HgSource {
    type Err = Error;

    fn from_str(mut s: &str) -> Result<Self> {
        let mut revision = None;

        if let Some((remaining, value)) = s.rsplit_once("#revision=") {
            revision = Some(value.to_string());
            s = remaining;
        }

        Ok(Self {
            url: s.to_string(),
            revision,
        })
    }
}
