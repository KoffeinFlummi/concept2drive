use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct FirmwareVersions {
    pub data: Vec<FirmwareVersion>
}

#[derive(Debug, Clone, Deserialize)]
pub struct FirmwareVersion {
    pub bug_fixes: String,
    pub description: String,
    pub internal_notes: String,
    pub machine: String,
    pub major_version: u32,
    pub minor_version: u32,
    pub monitor: String,
    pub new_features: String,
    pub optional: bool,
    pub release_date: String,
    pub short_description: String,
    pub status: String,
    pub version: f32,
    pub files: Vec<FirmwareFile>
}

#[derive(Debug, Clone, Deserialize)]
pub struct FirmwareFile {
    pub default: bool,
    pub languages: Vec<std::collections::HashMap<String,String>>,
    pub name: String,
    pub path: String,
    pub uploaded: String
}

impl FirmwareVersions {
    pub async fn download() -> Result<Self, reqwest::Error> {
        let resp = reqwest::Client::new()
            .get("https://tech.concept2.com/api/firmware/latest")
            .header("Authorization", "Basic Y29uY2VwdDJmaXJtd2FyZTpDKClyYnluMG0xYzU=")
            .send().await?;

        Ok(resp.json::<Self>().await?)
    }
}
