use proton::{
    downloaders::MinecraftDownloader, errors::ProtonError, manifest::resolve_version_data,
    types::DownloadProgress,
};
use tokio::{sync::mpsc, task};
#[tokio::main]
async fn main() -> Result<(), ProtonError> {
    let (tx, mut rx) = mpsc::channel::<DownloadProgress>(100);
    let mut downloader = MinecraftDownloader::new(
        std::env::current_dir()?.join("minecraft"),
        resolve_version_data("1.16.5".to_string()).await?,
    );

    task::spawn(async move {
        while let Some(msg) = rx.recv().await {
            println!("{:?}", msg)
        }
    });
    downloader.download_natives(Some(tx)).await?;
    Ok(())
}
