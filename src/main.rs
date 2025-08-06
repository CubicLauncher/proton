use std::path::PathBuf;

use proton::MinecraftDownloader;
use proton::resolve_version_data;

#[tokio::main]
async fn main() {
    let mut downloader = MinecraftDownloader::new(
        PathBuf::from("/home/santiagolxx/Documentos/cubic/proton/minecraft"),
        resolve_version_data("1.21.8".to_string()).await.unwrap(),
    );

    downloader.download_all(None).await.unwrap();
}
