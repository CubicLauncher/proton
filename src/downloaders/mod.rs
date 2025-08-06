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

const MAX_CONCURRENT_DOWNLOADS: usize = 128;

/// Macro para crear la estructura básica de descarga concurrente
macro_rules! create_download_infrastructure {
    ($total:expr, $game_version:expr) => {{
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS));
        let completed = Arc::new(AtomicUsize::new(0));
        let tasks = FuturesUnordered::new();
        let game_version = Arc::new($game_version.clone());
        (semaphore, completed, tasks, game_version, $total)
    }};
}

/// Macro para crear una tarea de descarga
macro_rules! create_download_task {
    (
        $tasks:expr,
        $semaphore:expr,
        $completed:expr,
        $progress_tx:expr,
        $game_version:expr,
        $total:expr,
        $download_type:expr,
        $name:expr,
        $url:expr,
        $path:expr,
        $hash:expr,
        $post_process:expr
    ) => {
        let semaphore = Arc::clone(&$semaphore);
        let completed = Arc::clone(&$completed);
        let tx = $progress_tx.clone();
        let game_version = Arc::clone(&$game_version);
        let info = DownloadProgressInfo {
            name: $name,
            version: game_version.clone(),
        };

        $tasks.push(tokio::spawn(async move {
            let permit = semaphore.acquire_owned().await;
            let result = download_file($url, &$path, $hash).await;

            // Ejecutar post-procesamiento específico
            $post_process?;

            let count = completed.fetch_add(1, Ordering::Relaxed) + 1;

            if let Some(tx) = tx {
                let _ = tx
                    .send(DownloadProgress {
                        current: count,
                        total: $total,
                        info,
                        download_type: $download_type,
                    })
                    .await;
            }
            drop(permit);
            result
        }));
    };
}

/// Macro para ejecutar todas las tareas y manejar errores
macro_rules! await_all_tasks {
    ($tasks:expr) => {
        while let Some(res) = $tasks.next().await {
            res??;
        }
    };
}

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

    /// Método principal que ejecuta todas las descargas en paralelo usando threads
    pub async fn download_all(
        &mut self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        // Crear canales para cada tipo de descarga si se proporciona progress_tx
        let (natives_tx, libraries_tx, assets_tx, client_tx) = if progress_tx.is_some() {
            let tx = progress_tx.as_ref().unwrap();
            (
                Some(tx.clone()),
                Some(tx.clone()),
                Some(tx.clone()),
                Some(tx.clone()),
            )
        } else {
            (None, None, None, None)
        };

        // Lanzar todas las descargas en paralelo
        let natives_handle = {
            let mut downloader = self.clone_for_natives();
            tokio::spawn(async move { downloader.download_natives_internal(natives_tx).await })
        };

        let libraries_handle = {
            let mut downloader = self.clone_for_libraries();
            tokio::spawn(async move { downloader.download_libraries_internal(libraries_tx).await })
        };

        let assets_handle = {
            let downloader = self.clone_for_assets();
            tokio::spawn(async move { downloader.download_assets_internal(assets_tx).await })
        };

        let client_handle = {
            let downloader = self.clone_for_client();
            tokio::spawn(async move { downloader.download_client_internal(client_tx).await })
        };

        // Esperar a que todas las descargas terminen
        let (natives_result, libraries_result, assets_result, client_result) = tokio::join!(
            natives_handle,
            libraries_handle,
            assets_handle,
            client_handle
        );

        // Verificar resultados
        natives_result??;
        libraries_result??;
        assets_result??;
        client_result??;

        Ok(())
    }

    // Métodos auxiliares para clonar el estado necesario para cada thread
    fn clone_for_natives(&self) -> MinecraftDownloader {
        let mut cloned =
            MinecraftDownloader::new(self.game_path.clone(), self.game_version.clone());
        cloned.game_version.natives = self.game_version.natives.clone();
        cloned
    }

    fn clone_for_libraries(&self) -> MinecraftDownloader {
        let mut cloned =
            MinecraftDownloader::new(self.game_path.clone(), self.game_version.clone());
        cloned.game_version.libraries = self.game_version.libraries.clone();
        cloned
    }

    fn clone_for_assets(&self) -> MinecraftDownloader {
        MinecraftDownloader::new(self.game_path.clone(), self.game_version.clone())
    }

    fn clone_for_client(&self) -> MinecraftDownloader {
        MinecraftDownloader::new(self.game_path.clone(), self.game_version.clone())
    }

    // Implementaciones internas de descarga
    async fn download_natives_internal(
        &mut self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        let natives = std::mem::take(&mut self.game_version.natives);
        let total = natives.len();
        let (semaphore, completed, mut tasks, game_version_arc, _) =
            create_download_infrastructure!(total, self.game_version.id);

        let natives_dir = Arc::new(self.natives_dir.clone());
        let temp_dir = self
            .game_path
            .join("temp")
            .join("natives")
            .join(uuid::Uuid::new_v4().to_string());

        tokio::fs::create_dir_all(&temp_dir).await?;

        for native in natives {
            let temp_native_path = temp_dir.join(&native.path);
            let natives_dir_clone = Arc::clone(&natives_dir);
            let temp_path_for_task = temp_native_path.clone();

            create_download_task!(
                tasks,
                semaphore,
                completed,
                progress_tx,
                game_version_arc,
                total,
                DownloadProgressType::Native,
                native.name,
                native.url,
                temp_native_path,
                native.sha1,
                extract_native(&temp_path_for_task, natives_dir_clone.as_ref()).await
            );
        }

        await_all_tasks!(tasks);
        tokio::fs::remove_dir_all(temp_dir).await?;
        Ok(())
    }

    async fn download_libraries_internal(
        &mut self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        let libraries = std::mem::take(&mut self.game_version.libraries);
        let total = libraries.len();
        let (semaphore, completed, mut tasks, game_version_arc, _) =
            create_download_infrastructure!(total, self.game_version.id);

        for library in libraries {
            let library_path = self.libraries_dir.join(&library.path);

            create_download_task!(
                tasks,
                semaphore,
                completed,
                progress_tx,
                game_version_arc,
                total,
                DownloadProgressType::Library,
                library.name,
                library.url,
                library_path,
                library.sha1,
                Ok::<(), ProtonError>(())
            );
        }

        await_all_tasks!(tasks);
        Ok(())
    }

    async fn download_assets_internal(
        &self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        let asset_index = resolve_asset_index(&self.game_version).await?;
        let total = asset_index.len();
        let (semaphore, completed, mut tasks, game_version_arc, _) =
            create_download_infrastructure!(total, self.game_version.id);

        for (name, asset) in asset_index.as_vec() {
            let hash = &asset.hash;
            let subhash: String = hash.chars().take(2).collect();
            let url = format!("{}/{}/{}", RESOURCES_BASE_URL, subhash, hash);
            let path = self.objects_dir.join(&subhash).join(hash);
            let hash_string = hash.to_string();

            create_download_task!(
                tasks,
                semaphore,
                completed,
                progress_tx,
                game_version_arc,
                total,
                DownloadProgressType::Asset,
                name,
                url,
                path,
                hash_string,
                Ok::<(), ProtonError>(())
            );
        }

        await_all_tasks!(tasks);
        Ok(())
    }

    async fn download_client_internal(
        &self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        // Clonar los datos necesarios para evitar problemas de lifetime
        let client_info = self.game_version.client_jar.clone();
        let total = 1;
        let (semaphore, completed, mut tasks, game_version_arc, _) =
            create_download_infrastructure!(total, self.game_version.id);

        let client_path = self
            .game_path
            .join("versions")
            .join(&self.game_version.id)
            .join(format!("{}.jar", self.game_version.id));

        create_download_task!(
            tasks,
            semaphore,
            completed,
            progress_tx,
            game_version_arc,
            total,
            DownloadProgressType::Client,
            format!("minecraft-{}", self.game_version.id),
            client_info.url,
            client_path,
            client_info.sha1,
            Ok::<(), ProtonError>(())
        );

        await_all_tasks!(tasks);
        Ok(())
    }
}
