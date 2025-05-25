use std::path::PathBuf;
use log::warn;
use once_cell::sync::Lazy;
use reqwest::Client;
use ring::digest::{Context, SHA1_FOR_LEGACY_USE_ONLY};
use tokio::{fs::{create_dir_all, remove_file, rename, File}, io::AsyncWriteExt};
use hex;
use futures::TryStreamExt;
use crate::errors::ProtonError;

pub static HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .user_agent("Cubic Proton/1.0")
        .build()
        .expect("Failed to build reqwest client")
});


const MAX_DOWNLOAD_ATTEMPTS: usize = 3;

pub async fn download_file(
    url: &str,
    path: PathBuf,
    expected_hash: &str,
) -> Result<(), ProtonError> {
    let temp_file = path.with_extension("tmp");
    for _attempt in 1..=MAX_DOWNLOAD_ATTEMPTS {
        let response = HTTP_CLIENT
            .get(url)
            .send()
            .await
            .map_err(|e|ProtonError::RequestError(e))?;

        // Crea el directorio de destino si no existe
        if let Some(parent_dir) = path.parent() {
            create_dir_all(parent_dir).await?;
        }

        // Crea archivo de destino
        let mut file = File::create(&temp_file).await.map_err(|e|ProtonError::IoError(e))?;

        // Prepara para cálculo de hash SHA1
        let mut sha1_context = Context::new(&SHA1_FOR_LEGACY_USE_ONLY);
        let mut stream = response.bytes_stream();

        // Escribe archivo en disco y actualiza hash en paralelo
        while let Some(chunk) = stream.try_next().await? {
            sha1_context.update(&chunk);
            file.write_all(&chunk).await?;
        }

        // Verifica el hash
        let actual_hash = hex::encode(sha1_context.finish());

        if &actual_hash == expected_hash {
            rename(temp_file, path).await?;
            return Ok(());
        } else {
            warn!(
                "HashMismatch error: EXPECTED: {}, OBTAINED: {}",
                expected_hash, actual_hash
            );
            // Elimina archivo corrupto
            let _ = remove_file(&temp_file).await;
        }
    }

    Err(ProtonError::HashMismatch)
}