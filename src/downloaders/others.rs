use crate::{
    errors::ProtonError,
    manifest::{resolve_version_data, resolve_version_in_manifest},
    utilities::download_file,
};
use std::path::PathBuf;

pub async fn download_version_json(
    version_id: String,
    gamedir: &PathBuf,
) -> Result<(), ProtonError> {
    let manifest = resolve_version_in_manifest(version_id.clone()).await?;
    download_file(
        manifest.url,
        gamedir
            .join("versions")
            .join(&version_id)
            .join(format!("{}.json", &version_id)),
        manifest.sha1,
    )
    .await?;
    Ok(())
}

pub async fn download_asset_index(
    version_id: String,
    gamedir: &PathBuf,
) -> Result<(), ProtonError> {
    let manifest = resolve_version_data(version_id.clone()).await?;
    download_file(
        manifest.asset_index.url,
        gamedir
            .join("assets")
            .join("indexes")
            .join(format!("{}.json", &manifest.asset_index.id)),
        manifest.asset_index.sha1,
    )
    .await?;
    Ok(())
}