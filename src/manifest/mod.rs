use crate::errors::ProtonError;
use crate::types::{MinecraftVersion, NormalizedVersion, VersionAssets, VersionManifest, MANIFEST_URL, VERSION_INDEX_URL};
use crate::utilities::HTTP_CLIENT;

pub async fn get_manifest() -> Result<VersionManifest, ProtonError> {
    let res = HTTP_CLIENT
        .get(MANIFEST_URL)
        .send()
        .await?
        .json::<VersionManifest>()
        .await?;
    Ok(res)
}

pub async fn resolve_version_in_manifest(version_id: String) -> Result<MinecraftVersion, ProtonError> {
    let manifest = get_manifest().await?;

    manifest
        .versions
        .iter()
        .find(|v| v.id == version_id)
        .cloned()
        .ok_or(ProtonError::VersionNotFound(version_id))
}

pub async fn resolve_version_data(version_id: String) -> Result<NormalizedVersion, ProtonError> {
    let version = HTTP_CLIENT.get(format!("{}/{}", VERSION_INDEX_URL, version_id)).send().await?.json::<NormalizedVersion>().await?;
    Ok(version)
}

pub async fn resolve_asset_index(game_version: &NormalizedVersion) -> Result<VersionAssets, ProtonError> {
    let res = HTTP_CLIENT
        .get(&game_version.asset_index.url)
        .send()
        .await?
        .json::<VersionAssets>()
        .await?;
    Ok(res)
}