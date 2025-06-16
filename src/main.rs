use std::path::PathBuf;
use proton::errors::ProtonError;
use proton::manifest::resolve_version_data;
use proton::downloaders::MinecraftDownloader;
use proton::types::DownloadProgress;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), ProtonError> {
    let (tx, mut rx) = mpsc::channel::<DownloadProgress>(100);
    better_panic::install();
    let path: PathBuf = PathBuf::from(std::env::current_dir().unwrap()).join("Minecraft");

    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            println!("[{:?}/{:?}] {:?}", message.current, message.total, message.name)
        }
    });
    Ok(())
}