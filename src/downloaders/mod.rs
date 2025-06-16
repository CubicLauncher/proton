use crate::errors::ProtonError;
use crate::manifest::resolve_asset_index;
use crate::types::{DownloadProgress, NormalizedVersion, RESOURCES_BASE_URL};
use crate::utilities::{download_file, extract_native};
use futures::stream::{FuturesUnordered, StreamExt};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Semaphore;
use tokio::sync::mpsc::Sender;
mod others;

const MAX_DOWNLOAD_ATTEMPTS: usize = 24;

pub struct MinecraftDownloader {
    game_path: PathBuf,
    game_version: NormalizedVersion,
}

impl MinecraftDownloader {
    pub fn new(path: PathBuf, game_version: NormalizedVersion) -> Self {
        Self {
            game_path: path,
            game_version,
        }
    }

    pub async fn download_libraries(
        &mut self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        let semaphore = Arc::new(Semaphore::new(MAX_DOWNLOAD_ATTEMPTS));
        let counter = Arc::new(AtomicUsize::new(0));
        let total = self.game_version.libraries.len();
        let mut tasks = FuturesUnordered::new();
        
        // Extraer datos necesarios y limpiar self inmediatamente
        let libraries = std::mem::take(&mut self.game_version.libraries);
        let libraries_base_path = self.game_path.join("libraries");
        
        for library in libraries {
            let semaphore = Arc::clone(&semaphore);
            let counter = Arc::clone(&counter);
            let libraries_base_path = libraries_base_path.clone();
            let progress_tx = progress_tx.clone();

            tasks.push(tokio::spawn(async move {
                let permit = semaphore.acquire_owned().await.unwrap();
                let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
                
                // Extraer campos necesarios usando referencias
                let library_path = libraries_base_path.join(&library.path);
                let library_url = library.url.clone();
                let library_sha1 = library.sha1.clone();
                let library_name = library.path.clone();
                
                let result = download_file(library_url, library_path, library_sha1).await;

                // Enviar progreso solo si es necesario
                if let Some(tx) = progress_tx {
                    let progress = DownloadProgress {
                        current,
                        total,
                        name: Some(library_name),
                    };
                    let _ = tx.send(progress).await;
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

    pub async fn download_natives(
        &mut self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        let semaphore = Arc::new(Semaphore::new(MAX_DOWNLOAD_ATTEMPTS));
        let counter = Arc::new(AtomicUsize::new(0));
        let total = self.game_version.natives.len();
        let mut tasks = FuturesUnordered::new();
        
        // Extraer y precalcular paths
        let natives = std::mem::take(&mut self.game_version.natives);
        let libraries_base_path = self.game_path.join("libraries");
        let natives_extraction_path = self.game_path.join("natives").join(&self.game_version.id);

        for native in natives {
            let semaphore = Arc::clone(&semaphore);
            let counter = Arc::clone(&counter);
            let libraries_base_path = libraries_base_path.clone();
            let natives_extraction_path = natives_extraction_path.clone();
            let progress_tx = progress_tx.clone();

            tasks.push(tokio::spawn(async move {
                let permit = semaphore.acquire_owned().await.unwrap();
                let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
                
                // Extraer campos necesarios usando clones mínimos
                let native_path = libraries_base_path.join(&native.path);
                let native_url = native.url.clone();
                let native_sha1 = native.sha1.clone();
                let native_name = native.path.clone();
                
                let result = download_file(native_url, native_path.clone(), native_sha1).await;

                if let Some(tx) = progress_tx {
                    let progress = DownloadProgress {
                        current,
                        total,
                        name: Some(native_name),
                    };
                    let _ = tx.send(progress).await;
                }
                
                // Extraer después de descargar
                if result.is_ok() {
                    extract_native(&native_path, &natives_extraction_path).await?;
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
        &mut self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        let semaphore = Arc::new(Semaphore::new(MAX_DOWNLOAD_ATTEMPTS));
        let counter = Arc::new(AtomicUsize::new(0));
        
        // Resolver asset index
        let asset_index = resolve_asset_index(&self.game_version).await?;
        let total = asset_index.objects.len();
        let mut tasks = FuturesUnordered::new();
        
        // Precalcular path base
        let assets_objects_path = self.game_path.join("assets").join("objects");
        
        // Precalcular capacidad de URL para evitar realocaciones
        let url_base_capacity = RESOURCES_BASE_URL.len() + 70; // Buffer para hash + separadores

        for (name, asset) in asset_index.objects {
            let semaphore = Arc::clone(&semaphore);
            let counter = Arc::clone(&counter);
            let assets_objects_path = assets_objects_path.clone();
            let progress_tx = progress_tx.clone();

            tasks.push(tokio::spawn(async move {
                let permit = semaphore.acquire_owned().await.unwrap();
                let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
                
                // Usar slice para subhash sin allocación
                let asset_hash = &asset.hash;
                let asset_subhash = &asset_hash[..2];
                
                // Construir path de forma eficiente
                let asset_path = assets_objects_path
                    .join(asset_subhash)
                    .join(asset_hash);
                
                // Construir URL de forma eficiente sin format!
                let mut asset_url = String::with_capacity(url_base_capacity);
                asset_url.push_str(RESOURCES_BASE_URL);
                asset_url.push('/');
                asset_url.push_str(asset_subhash);
                asset_url.push('/');
                asset_url.push_str(asset_hash);
                
                let asset_hash_owned = asset.hash;
                
                let result = download_file(asset_url, asset_path, asset_hash_owned).await;

                if let Some(tx) = progress_tx {
                    let progress = DownloadProgress {
                        current,
                        total,
                        name: Some(name),
                    };
                    let _ = tx.send(progress).await;
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