use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::errors::ProtonError;
use crate::types::{
    MOJANG_MANIFEST_URL, MojangVersionDetails, MojangVersionInfo, MojangVersionManifest,
    NormalizedVersion, VersionAssets,
};
use crate::utilities::HTTP_CLIENT;

pub async fn get_manifest() -> Result<MojangVersionManifest, ProtonError> {
    let res = HTTP_CLIENT
        .get(MOJANG_MANIFEST_URL)
        .send()
        .await?
        .json::<MojangVersionManifest>()
        .await?;
    Ok(res)
}

pub async fn resolve_version_in_manifest(
    version_id: String,
) -> Result<MojangVersionInfo, ProtonError> {
    let manifest = get_manifest().await?;

    manifest
        .versions
        .into_iter()
        .find(|v| v.id == version_id)
        .ok_or(ProtonError::VersionNotFound(version_id))
}

pub async fn resolve_version_data(version_id: String) -> Result<NormalizedVersion, ProtonError> {
    let version_manifest = HTTP_CLIENT
        .get(MOJANG_MANIFEST_URL)
        .send()
        .await?
        .json::<MojangVersionManifest>()
        .await?;

    let version = version_manifest
        .versions
        .par_iter()
        .find_any(|version| version.id == version_id)
        .cloned()
        .ok_or(ProtonError::VersionNotFound(version_id))?;

    let version = HTTP_CLIENT
        .get(version.url)
        .send()
        .await?
        .json::<MojangVersionDetails>()
        .await?;
    NormalizedVersion::try_from(version)
}

pub async fn resolve_asset_index(
    version: &NormalizedVersion,
) -> Result<VersionAssets, ProtonError> {
    let res = HTTP_CLIENT
        .get(&version.asset_index.url)
        .send()
        .await?
        .json::<VersionAssets>()
        .await?;
    Ok(res)
}
