use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Output {
    pub results: Option<Vec<ScanResult>>,
}

#[derive(Debug, Deserialize)]
pub struct ScanResult {
    pub source: Source,
    pub packages: Vec<Packages>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Packages {
    pub package: Package,
    pub vulnerabilities: Vec<Vulnerability>,
    pub groups: Option<Vec<Group>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Source {
    pub path: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: Option<String>,
    pub ecosystem: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Vulnerability {
    pub id: String,
    pub aliases: Option<Vec<String>>,
    pub summary: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Group {
    pub ids: Vec<String>,
}
