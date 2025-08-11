use proton::{DownloadProgress, DownloadProgressType, MinecraftDownloader, resolve_version_data};
use std::path::PathBuf;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel::<DownloadProgress>(100);

    let progress_handle = tokio::spawn(async move {
        while let Some(progress) = rx.recv().await {
            match progress.download_type {
                DownloadProgressType::Client => println!(
                    "Descargando cliente: {}/{}",
                    progress.current, progress.total
                ),
                DownloadProgressType::Library => println!(
                    "Descargando librería: {}/{}",
                    progress.current, progress.total
                ),
                DownloadProgressType::Asset => {
                    println!("Descargando asset: {}/{}", progress.current, progress.total)
                }
                DownloadProgressType::Native => println!(
                    "Descargando nativo: {}/{}",
                    progress.current, progress.total
                ),
                DownloadProgressType::Manifest => println!(
                    "Descargando Manifesto: {}/{}",
                    progress.current, progress.total
                ),
            }
        }
    });

    let mut downloader = MinecraftDownloader::new(
        PathBuf::from("/tmp/minecraft"),
        resolve_version_data("1.21.8").await.unwrap(),
    );

    downloader.download_all(Some(tx)).await.unwrap();

    // Esperar a que termine el lector de progreso
    progress_handle.await.unwrap();
    let (current, min, max) = downloader.get_download_stats().await;
    println!("Concurrencia final: {}/{}/{}", current, min, max);
}
