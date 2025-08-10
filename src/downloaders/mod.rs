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
use std::time::{Duration, Instant};
use tokio::sync::mpsc::Sender;
use tokio::sync::{Mutex, Semaphore};

/// Configuración adaptativa de descargas
struct AdaptiveConfig {
    max_concurrent: usize,
    current_concurrent: usize,
    min_concurrent: usize,
    performance_samples: Vec<Duration>,
    last_adjustment: Instant,
    sample_size: usize,
    performance_threshold_ms: u64,
    adjustment_interval_secs: u64,
}

impl AdaptiveConfig {
    fn new() -> Self {
        let max_concurrent = calculate_optimal_downloads();
        Self {
            max_concurrent,
            current_concurrent: (max_concurrent / 2).max(4),
            min_concurrent: 4,
            performance_samples: Vec::with_capacity(10),
            last_adjustment: Instant::now(),
            sample_size: 8,
            performance_threshold_ms: 1000,
            adjustment_interval_secs: 5,
        }
    }

    fn conservative() -> Self {
        let mut config = Self::new();
        config.max_concurrent = config.max_concurrent / 2;
        config.current_concurrent = 4;
        config.min_concurrent = 2;
        config.performance_threshold_ms = 2000;
        config
    }

    fn aggressive() -> Self {
        let mut config = Self::new();
        config.max_concurrent = config.max_concurrent * 2;
        config.current_concurrent = config.max_concurrent / 2;
        config.min_concurrent = 8;
        config.performance_threshold_ms = 500;
        config
    }

    fn record_and_adjust(&mut self, duration: Duration) {
        self.performance_samples.push(duration);

        if self.performance_samples.len() > self.sample_size {
            self.performance_samples.remove(0);
        }

        if self.last_adjustment.elapsed().as_secs() >= self.adjustment_interval_secs
            && self.performance_samples.len() >= self.sample_size / 2
        {
            self.adjust_concurrency();
        }
    }

    fn adjust_concurrency(&mut self) {
        if self.performance_samples.is_empty() {
            return;
        }

        let total_ms: u128 = self.performance_samples.iter().map(|d| d.as_millis()).sum();
        let avg_ms = total_ms / self.performance_samples.len() as u128;

        if avg_ms > self.performance_threshold_ms as u128 {
            // Rendimiento bajo, reducir concurrencia
            self.current_concurrent = (self.current_concurrent * 8 / 10).max(self.min_concurrent);
        } else if avg_ms < (self.performance_threshold_ms / 2) as u128 {
            // Buen rendimiento, aumentar concurrencia
            self.current_concurrent = (self.current_concurrent * 11 / 10).min(self.max_concurrent);
        }

        self.last_adjustment = Instant::now();
        self.performance_samples.clear();
    }
}

/// Calcula el número óptimo de descargas basado en el sistema
fn calculate_optimal_downloads() -> usize {
    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let memory_gb = get_available_memory_gb();

    // Algoritmo híbrido: CPU cores * 6 + memoria en GB * 4
    let cpu_based = cpu_cores * 6;
    let memory_based = (memory_gb * 4.0) as usize;

    // Tomar el mínimo para evitar saturación, con límites seguros
    cpu_based.min(memory_based).clamp(8, 256)
}

/// Obtiene memoria disponible aproximada en GB
fn get_available_memory_gb() -> f64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            for line in meminfo.lines() {
                if line.starts_with("MemAvailable:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            return (kb as f64) / (1024.0 * 1024.0);
                        }
                    }
                }
            }
        }
    }

    // Fallback para otros sistemas
    8.0
}

/// Macro para crear infraestructura de descarga adaptativa
macro_rules! create_adaptive_infrastructure {
    ($total:expr, $game_version:expr, $config:expr) => {{
        let current_limit = $config.lock().await.current_concurrent;
        let semaphore = Arc::new(Semaphore::new(current_limit));
        let completed = Arc::new(AtomicUsize::new(0));
        let tasks = FuturesUnordered::new();
        let game_version = Arc::new($game_version.clone());
        (semaphore, completed, tasks, game_version, $total)
    }};
}

/// Macro para crear tarea de descarga con monitoreo
macro_rules! create_monitored_task {
    (
        $tasks:expr,
        $semaphore:expr,
        $completed:expr,
        $progress_tx:expr,
        $game_version:expr,
        $config:expr,
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
        let config = Arc::clone(&$config);
        let tx = $progress_tx.clone();
        let game_version = Arc::clone(&$game_version);
        let info = DownloadProgressInfo {
            name: $name,
            version: game_version.clone(),
        };

        $tasks.push(tokio::spawn(async move {
            let start_time = Instant::now();
            let permit = semaphore
                .acquire_owned()
                .await
                .map_err(|_| ProtonError::Other("Failed to acquire download permit".to_string()))?;

            let result = download_file($url, &$path, $hash).await;
            let download_duration = start_time.elapsed();

            // Registrar tiempo para ajuste adaptativo
            {
                let mut config_guard = config.lock().await;
                config_guard.record_and_adjust(download_duration);
            }

            // Post-procesamiento
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

pub struct MinecraftDownloader {
    game_path: PathBuf,
    game_version: NormalizedVersion,
    natives_dir: PathBuf,
    objects_dir: PathBuf,
    libraries_dir: PathBuf,
    adaptive_config: Arc<Mutex<AdaptiveConfig>>,
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
            adaptive_config: Arc::new(Mutex::new(AdaptiveConfig::new())),
        }
    }

    /// Constructor con configuración personalizada
    pub fn with_config(
        game_path: PathBuf,
        game_version: NormalizedVersion,
        aggressive: bool,
    ) -> Self {
        let mut downloader = Self::new(game_path, game_version);
        downloader.adaptive_config = Arc::new(Mutex::new(if aggressive {
            AdaptiveConfig::aggressive()
        } else {
            AdaptiveConfig::conservative()
        }));
        downloader
    }

    /// Método principal con descarga adaptativa
    pub async fn download_all(
        &mut self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        println!(
            "Starting adaptive downloads with initial concurrency: {}",
            self.adaptive_config.lock().await.current_concurrent
        );

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

        // Clonar configuración para cada hilo
        let natives_config = Arc::clone(&self.adaptive_config);
        let libraries_config = Arc::clone(&self.adaptive_config);
        let assets_config = Arc::clone(&self.adaptive_config);
        let client_config = Arc::clone(&self.adaptive_config);

        let natives_handle = {
            let mut downloader = self.clone_for_natives();
            downloader.adaptive_config = natives_config;
            tokio::spawn(async move { downloader.download_natives_internal(natives_tx).await })
        };

        let libraries_handle = {
            let mut downloader = self.clone_for_libraries();
            downloader.adaptive_config = libraries_config;
            tokio::spawn(async move { downloader.download_libraries_internal(libraries_tx).await })
        };

        let assets_handle = {
            let mut downloader = self.clone_for_assets();
            downloader.adaptive_config = assets_config;
            tokio::spawn(async move { downloader.download_assets_internal(assets_tx).await })
        };

        let client_handle = {
            let mut downloader = self.clone_for_client();
            downloader.adaptive_config = client_config;
            tokio::spawn(async move { downloader.download_client_internal(client_tx).await })
        };

        let (natives_result, libraries_result, assets_result, client_result) = tokio::join!(
            natives_handle,
            libraries_handle,
            assets_handle,
            client_handle
        );

        natives_result??;
        libraries_result??;
        assets_result??;
        client_result??;

        let final_config = self.adaptive_config.lock().await;
        println!(
            "Downloads completed with final concurrency: {}",
            final_config.current_concurrent
        );

        Ok(())
    }

    async fn download_natives_internal(
        &mut self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        let natives = std::mem::take(&mut self.game_version.natives);
        let total = natives.len();
        let (semaphore, completed, mut tasks, game_version_arc, _) =
            create_adaptive_infrastructure!(total, self.game_version.id, self.adaptive_config);

        let natives_dir = Arc::new(self.natives_dir.clone());
        let temp_dir = self
            .game_path
            .join("temp")
            .join("natives")
            .join(format!("native_temp_{}", std::process::id()));

        tokio::fs::create_dir_all(&temp_dir).await?;

        for native in natives {
            let temp_native_path = temp_dir.join(&native.path);
            let natives_dir_clone = Arc::clone(&natives_dir);
            let temp_path_for_task = temp_native_path.clone();

            create_monitored_task!(
                tasks,
                semaphore,
                completed,
                progress_tx,
                game_version_arc,
                self.adaptive_config,
                total,
                DownloadProgressType::Native,
                native.name,
                native.url,
                temp_native_path,
                native.sha1,
                extract_native(&temp_path_for_task, natives_dir_clone.as_ref()).await
            );
        }

        while let Some(res) = tasks.next().await {
            res??;
        }

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
            create_adaptive_infrastructure!(total, self.game_version.id, self.adaptive_config);

        for library in libraries {
            let library_path = self.libraries_dir.join(&library.path);

            create_monitored_task!(
                tasks,
                semaphore,
                completed,
                progress_tx,
                game_version_arc,
                self.adaptive_config,
                total,
                DownloadProgressType::Library,
                library.name,
                library.url,
                library_path,
                library.sha1,
                Ok::<(), ProtonError>(())
            );
        }

        while let Some(res) = tasks.next().await {
            res??;
        }
        Ok(())
    }

    async fn download_assets_internal(
        &self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        let asset_index = resolve_asset_index(&self.game_version).await?;
        let total = asset_index.len();
        let (semaphore, completed, mut tasks, game_version_arc, _) =
            create_adaptive_infrastructure!(total, self.game_version.id, self.adaptive_config);

        for (name, asset) in asset_index.as_vec() {
            let hash = &asset.hash;
            let subhash: String = hash.chars().take(2).collect();
            let url = format!("{}/{}/{}", RESOURCES_BASE_URL, subhash, hash);
            let path = self.objects_dir.join(&subhash).join(hash);
            let hash_string = hash.to_string();

            create_monitored_task!(
                tasks,
                semaphore,
                completed,
                progress_tx,
                game_version_arc,
                self.adaptive_config,
                total,
                DownloadProgressType::Asset,
                name,
                url,
                path,
                hash_string,
                Ok::<(), ProtonError>(())
            );
        }

        while let Some(res) = tasks.next().await {
            res??;
        }
        Ok(())
    }

    async fn download_client_internal(
        &self,
        progress_tx: Option<Sender<DownloadProgress>>,
    ) -> Result<(), ProtonError> {
        let client_info = self.game_version.client_jar.clone();
        let total = 1;
        let (semaphore, completed, mut tasks, game_version_arc, _) =
            create_adaptive_infrastructure!(total, self.game_version.id, self.adaptive_config);

        let client_path = self
            .game_path
            .join("versions")
            .join(&self.game_version.id)
            .join(format!("{}.jar", self.game_version.id));

        create_monitored_task!(
            tasks,
            semaphore,
            completed,
            progress_tx,
            game_version_arc,
            self.adaptive_config,
            total,
            DownloadProgressType::Client,
            format!("minecraft-{}", self.game_version.id),
            client_info.url,
            client_path,
            client_info.sha1,
            Ok::<(), ProtonError>(())
        );

        while let Some(res) = tasks.next().await {
            res??;
        }
        Ok(())
    }

    // Métodos de clonación (sin cambios)
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

    /// Obtiene estadísticas actuales de la configuración adaptativa
    pub async fn get_download_stats(&self) -> (usize, usize, usize) {
        let config = self.adaptive_config.lock().await;
        (
            config.current_concurrent,
            config.min_concurrent,
            config.max_concurrent,
        )
    }
}
