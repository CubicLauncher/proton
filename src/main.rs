use std::path::PathBuf;

use proton::downloaders::{download_assets, download_client, others, download_libraries};
use proton::errors::ProtonError;
use proton::manifest::{resolve_asset_index, resolve_version_data};
use proton::types::DownloadProgress;
use tokio::sync::mpsc;


#[tokio::main]
async fn main() -> Result<(), ProtonError> {
    let path = PathBuf::from(format!("{}\\minecraft", std::env::current_dir().unwrap().display()));
    let a =resolve_version_data("1.21.5".to_string()).await?;

    let (tx, mut rx) = mpsc::channel::<DownloadProgress>(100);

    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let Some(name) = event.name {
                println!("Descargando: {} ({}/{})", name, event.current, event.total);
            } else {
                println!("Progreso: {}/{}", event.current, event.total);
            }
        }
    });

    others::download_asset_index("1.21.5".to_string(), &path).await?;

    Ok(())
}