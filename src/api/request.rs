use crate::api::error::ApiError;
use reqwest::header::HeaderMap;
use reqwest::Response;

/// For an reqwest response check the registry version as well as map errors to `ApiError`s
pub async fn handle_response(response: Response) -> Result<Response, ApiError> {
    validate_registry_version(&response)?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await?;
        Err(ApiError::RegistryError(body.trim().to_string()))
    } else {
        Ok(response)
    }
}

/// Validate the `Docker-Distribution-API-Version` header was present in the response and that it's value
/// is set to use registry v2
pub fn validate_registry_version(response: &Response) -> Result<(), ApiError> {
    if let Some(version) = response.headers().get("Docker-Distribution-API-Version") {
        if let Ok(parsed) = version.to_str() {
            if parsed.ends_with("/2.0") {
                Ok(())
            } else {
                Err(ApiError::UnsupportedRegistry)
            }
        } else {
            Err(ApiError::InvalidHeaderValue(String::from(
                "Docker-Distribution-API-Version",
            )))
        }
    } else {
        Ok(())
    }
}

/// For responses which use the `Link` header for pagination the header value
/// is read and parsed as proposed in RFC 5988
pub fn get_follow_path(headers: &HeaderMap) -> Result<Option<String>, ApiError> {
    if let Some(link) = headers.get(reqwest::header::LINK) {
        let link_str = link
            .to_str()
            .map_err(|_| ApiError::InvalidHeaderValue(String::from("Link")))?;
        let parts: Vec<&str> = link_str.split(';').collect();
        if let Some(url_part) = parts.first() {
            if let Some(path) = url_part
                .trim()
                .strip_prefix('<')
                .and_then(|s| s.strip_suffix('>'))
            {
                return Ok(Some(String::from(path)));
            }
        }
    }
    Ok(None)
}
