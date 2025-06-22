use crate::errors::ProtonError;
use crate::manifest::resolve_asset_index;
use crate::types::{
    DownloadProgress, DownloadProgressInfo, DownloadProgressType, NormalizedVersion,
    RESOURCES_BASE_URL,
};
use crate::utilities::{download_file, extract_native};
use futures::stream::{FuturesUnordered, StreamExt};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Semaphore;
use tokio::sync::mpsc::Sender;

const MAX_CONCURRENT_DOWNLOADS: usize = 24;

pub struct MinecraftDownloader {
    game_path: PathBuf,
    game_version: NormalizedVersion,
    natives_dir: PathBuf,
    objects_dir: PathBuf,
    libraries_dir: PathBuf,
}

impl MinecraftDownloader {
    pub fn new(game_path: PathBuf, game_version: NormalizedVersion) -> Self {
        let natives_dir = game_path.join("natives").join(&game_version.id);
        let objects_dir = game_path.join("assets").join("objects");
        let libraries_dir = game_path.join("libraries");
        Self {
            game_path,
            game_version,
            natives_dir,
            objects_dir,
            libraries_dir,
        }
    }

    pub async fn download_natives(
        &mut self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS));
        let total = self.game_version.natives.len();
        // Como no se usa mas prefiero tomarlo
        // att: santiagolxx
        let natives = std::mem::take(&mut self.game_version.natives);
        let completed = Arc::new(AtomicUsize::new(0));
        let mut tasks = FuturesUnordered::new();
        let natives_dir = Arc::new(self.natives_dir.clone());
        let game_version = Arc::new(self.game_version.id.clone());
        let temp_dir = &self
            .game_path
            .join("temp")
            .join(uuid::Uuid::new_v4().to_string());

        tokio::fs::create_dir_all(temp_dir).await?;

        for native in natives {
            let temp_native_path = temp_dir.join(native.path);
            let completed = Arc::clone(&completed);
            let semaphore = Arc::clone(&semaphore);
            let natives_dir = Arc::clone(&natives_dir);
            let tx = progress_tx.clone();
            let info = DownloadProgressInfo {
                name: native.name,
                version: Arc::clone(&game_version),
            };
            tasks.push(tokio::spawn(async move {
                let permit = semaphore.acquire_owned().await;
                let result = download_file(native.url, &temp_native_path, native.sha1).await;
                extract_native(&temp_native_path, natives_dir.as_ref()).await?;
                let count = completed.fetch_add(1, Ordering::Relaxed) + 1;

                if let Some(tx) = tx {
                    let _ = tx
                        .send(DownloadProgress {
                            current: count,
                            total,
                            info: info,
                            download_type: DownloadProgressType::Native,
                        })
                        .await;
                }
                drop(permit);
                result
            }));
        }
        while let Some(res) = tasks.next().await {
            res??;
        }
        tokio::fs::remove_dir_all(temp_dir).await?;
        Ok(())
    }

    pub async fn download_libraries(
        &mut self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS));
        let total = self.game_version.libraries.len();
        // Esto la verdad es lo mismo que el de natives
        // solo cambia que no se extraen los jars
        // ni tampoco se necesita el dir temporal.
        // att: santiagolxx
        let libraries = std::mem::take(&mut self.game_version.libraries);
        let completed = Arc::new(AtomicUsize::new(0));
        let mut tasks = FuturesUnordered::new();
        let game_version = Arc::new(self.game_version.id.clone());

        for library in libraries {
            let library_path = self.libraries_dir.join(library.path);
            let completed = Arc::clone(&completed);
            let semaphore = Arc::clone(&semaphore);
            let tx = progress_tx.clone();
            let info = DownloadProgressInfo {
                name: library.name,
                version: Arc::clone(&game_version),
            };

            tasks.push(tokio::spawn(async move {
                let permit = semaphore.acquire_owned().await;
                let result = download_file(library.url, &library_path, library.sha1).await;
                let count = completed.fetch_add(1, Ordering::Relaxed) + 1;

                if let Some(tx) = tx {
                    let _ = tx
                        .send(DownloadProgress {
                            current: count,
                            total,
                            info: info,
                            download_type: DownloadProgressType::Library,
                        })
                        .await;
                }
                drop(permit);
                result
            }));
        }
        while let Some(res) = tasks.next().await {
            res??;
        }
        Ok(())
    }

    pub async fn download_assets(
        &self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS));
        let asset_index = resolve_asset_index(&self.game_version).await?;
        let mut tasks = FuturesUnordered::new();
        let completed = Arc::new(AtomicUsize::new(0));
        let total = asset_index.len();
        let game_version = Arc::new(self.game_version.id.clone());

        for (name, asset) in asset_index.as_vec() {
            let hash = &asset.hash;
            let semaphore = Arc::clone(&semaphore);
            let subhash: String = hash.chars().take(2).collect();
            let url = format!("{}/{}/{}", RESOURCES_BASE_URL, subhash, hash);
            let path = self.objects_dir.join(&subhash).join(hash);
            let hash = hash.to_string();
            let tx = progress_tx.clone();
            let completed = Arc::clone(&completed);
            let info = DownloadProgressInfo {
                name: name,
                version: Arc::clone(&game_version),
            };

            tasks.push(tokio::spawn(async move {
                let permit = semaphore.acquire_owned().await;
                let result = download_file(url, &path, hash).await;
                let count = completed.fetch_add(1, Ordering::Relaxed) + 1;

                if let Some(tx) = tx {
                    let _ = tx
                        .send(DownloadProgress {
                            current: count,
                            total,
                            info: info,
                            download_type: DownloadProgressType::Asset,
                        })
                        .await;
                }
                drop(permit);
                result
            }));
        }

        while let Some(res) = tasks.next().await {
            res??;
        }

        Ok(())
    }
}
