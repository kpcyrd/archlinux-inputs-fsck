use crate::errors::*;
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BzrSource {
    url: String,
    revision: Option<String>,
}

impl FromStr for BzrSource {
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
