use crate::{
    errors::ProtonError,
    manifest::resolve_asset_index,
    types::{DownloadProgress, NormalizedVersion, RESOURCES_BASE_URL},
    utilities::{download_file, extract_native, get_os_name_runtime},
};
use futures::stream::{FuturesUnordered, StreamExt};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::{Semaphore, mpsc::Sender};
pub mod others;

const MAX_CONCURRENT_DOWNLOADS: usize = 16;

pub async fn download_client(
    version_data: &NormalizedVersion,
    gamedir: PathBuf,
) -> Result<(), ProtonError> {
    let client_jar = &version_data.client_jar;
    let mut client_path = gamedir.clone();
    client_path.push(format!("versions/{0}/{0}.jar", version_data.id));
    download_file(&client_jar.url, &client_path, &client_jar.sha1).await?;
    Ok(())
}

pub async fn download_assets(
    game_version: &NormalizedVersion,
    gamedir: &PathBuf,
    progress_tx: Option<Sender<DownloadProgress>>,
) -> Result<(), ProtonError> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS));
    let assets = resolve_asset_index(&game_version).await?;
    let counter = Arc::new(AtomicUsize::new(0));
    let total = assets.objects.len();
    let mut tasks = FuturesUnordered::new();

    for (_name, asset) in assets.objects {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let subhash: String = asset.hash.chars().take(2).collect();
        let url = format!("{}/{}/{}", RESOURCES_BASE_URL, subhash, asset.hash);
        let path = format!(
            "{}/assets/objects/{}/{}",
            gamedir.display(),
            subhash,
            asset.hash
        );
        let id = game_version.id.clone();
        let mut file_path = gamedir.clone();
        file_path.push("libraries");
        file_path.push(id);
        file_path.push(path.clone());

        let counter = counter.clone();
        let progress_tx = progress_tx.clone();

        tasks.push(tokio::spawn(async move {
            let result = download_file(&url, &file_path, &asset.hash).await;
            let current = counter.fetch_add(1, Ordering::SeqCst) + 1;

            if let Some(tx) = progress_tx {
                let _ = tx
                    .send(DownloadProgress {
                        current,
                        total,
                        name: Some(path), // opcionalmente envías el nombre del archivo
                    })
                    .await;
            }

            drop(permit);
            result
        }));
    }

    while let Some(res) = tasks.next().await {
        res.unwrap()?; // Propaga errores de descarga
    }

    Ok(())
}

pub async fn download_libraries(
    game_version: &NormalizedVersion,
    gamedir: &PathBuf,
    progress_tx: Option<Sender<DownloadProgress>>,
) -> Result<(), ProtonError> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS));
    let counter = Arc::new(AtomicUsize::new(0));
    let total = game_version.libraries.len();
    let mut tasks = FuturesUnordered::new();

    for library in &game_version.libraries {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let url = library.url.clone();
        let hash = library.sha1.clone();
        let path = library.path.clone();
        let id = game_version.id.clone();
        let mut file_path = gamedir.clone();
        file_path.push("libraries");
        file_path.push(path.clone());

        let counter = counter.clone();
        let progress_tx = progress_tx.clone();

        tasks.push(tokio::spawn(async move {
            let result = download_file(&url, &file_path, &hash).await;
            let current = counter.fetch_add(1, Ordering::SeqCst) + 1;

            if let Some(tx) = progress_tx {
                let _ = tx
                    .send(DownloadProgress {
                        current,
                        total,
                        name: Some(path), // opcionalmente envías el nombre del archivo
                    })
                    .await;
            }

            drop(permit);
            result
        }));
    }

    while let Some(res) = tasks.next().await {
        res.unwrap()?; // Propaga errores de descarga
    }

    Ok(())
}

pub async fn download_natives(
    game_version: &NormalizedVersion,
    gamedir: &PathBuf,
    progress_tx: Option<Sender<DownloadProgress>>,
) -> Result<(), ProtonError> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS));
    let counter = Arc::new(AtomicUsize::new(0));
    let total = game_version.natives.len();
    let mut tasks = FuturesUnordered::new();
    let os_name = get_os_name_runtime(); // se supone que tendria que ser macos, linux o windows, si es darwin entonces no se XD

    let expected_classifier = format!("natives-{}", os_name);
    for native in &game_version.natives {
        if native.classifier != expected_classifier {
            continue;
        }
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let url = native.url.clone();
        let hash = native.sha1.clone();
        let path = native.path.clone();
        let id = game_version.id.clone();
        let mut file_path = gamedir.clone();
        file_path.push("libraries");
        file_path.push(path.clone());
        let mut a = gamedir.clone();
        a.push("natives");
        a.push(&game_version.id);
        let counter = counter.clone();
        let progress_tx = progress_tx.clone();

        tasks.push(tokio::spawn(async move {
            let result = download_file(&url, &file_path, &hash).await;
            let current = counter.fetch_add(1, Ordering::SeqCst) + 1;

            if let Some(tx) = progress_tx {
                let _ = tx
                    .send(DownloadProgress {
                        current,
                        total,
                        name: Some(path), // opcionalmente envías el nombre del archivo
                    })
                    .await;
            }

            if result.is_ok() {
                if let Err(e) = extract_native(&file_path, &a).await {
                    eprintln!("❌ Error al extraer native {}", a.display());
                } else {
                    println!("📦 Extraído: {}", a.display());
                }
            }

            println!("{}", &a.display());

            drop(permit);
            result
        }));
    }

    while let Some(res) = tasks.next().await {
        res.unwrap()?; // Propaga errores de descarga
    }

    Ok(())
}
