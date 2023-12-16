use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Layer {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub digest: String,
    pub size: u32,
}
