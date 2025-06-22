use crate::errors::ProtonError;
use async_zip::tokio::read::fs::ZipFileReader;
use futures::TryStreamExt;
use hex;
use log::{error, info, warn};
use once_cell::sync::Lazy;
use reqwest::Client;
use ring::digest::{Context, SHA1_FOR_LEGACY_USE_ONLY};
use std::path::PathBuf;
use tokio::{
    fs::{File, create_dir_all, remove_file, rename},
    io::{AsyncReadExt, AsyncWriteExt},
    time::Duration,
};

pub static HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .user_agent("Cubic Proton/1.0")
        .build()
        .expect("Failed to build reqwest client")
});

const MAX_DOWNLOAD_ATTEMPTS: usize = 3;

pub async fn download_file(
    url: String,
    path: &PathBuf,
    expected_hash: String,
) -> Result<(), ProtonError> {
    // Validaciones iniciales
    if url.is_empty() || expected_hash.is_empty() {
        return Err(ProtonError::Other(
            "URL and hash cannot be empty".to_string(),
        ));
    }

    // Verificar si el archivo ya existe y tiene el hash correcto
    if path.exists() {
        info!("File already exists, verifying hash: {:?}", path);

        match verify_file_hash(path, &expected_hash).await {
            Ok(true) => {
                info!("File already exists with correct hash: {:?}", path);
                return Ok(());
            }
            Ok(false) => {
                warn!(
                    "File exists but hash doesn't match, re-downloading: {:?}",
                    path
                );
                // Eliminar archivo corrupto
                if let Err(e) = remove_file(path).await {
                    warn!("Failed to remove corrupted file: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to verify existing file hash: {}, re-downloading", e);
                // Eliminar archivo que no se puede verificar
                if let Err(e) = remove_file(path).await {
                    warn!("Failed to remove unverifiable file: {}", e);
                }
            }
        }
    }

    // Generar nombre único para archivo temporal
    let temp_file = path.with_extension(&format!("tmp.{}", uuid::Uuid::new_v4()));

    for attempt in 1..=MAX_DOWNLOAD_ATTEMPTS {
        // Crear directorio padre si no existe
        if let Some(parent_dir) = path.parent() {
            if let Err(e) = create_dir_all(parent_dir).await {
                error!("Failed to create directory {:?}: {}", parent_dir, e);
                return Err(ProtonError::IoError(e));
            }
        }

        // Limpiar archivo temporal si existe de intentos anteriores
        if temp_file.exists() {
            let _ = remove_file(&temp_file).await;
        }

        // Realizar petición HTTP
        let response = match HTTP_CLIENT.get(&url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    warn!("HTTP error on attempt {}: {}", attempt, resp.status());
                    continue;
                }
                resp
            }
            Err(e) => {
                warn!("Request failed on attempt {}: {}", attempt, e);
                if attempt == MAX_DOWNLOAD_ATTEMPTS {
                    return Err(ProtonError::RequestError(e));
                }
                continue;
            }
        };

        // Crear archivo temporal
        let mut file = match File::create(&temp_file).await {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to create temp file {:?}: {}", temp_file, e);
                return Err(ProtonError::IoError(e));
            }
        };

        // Prepara para cálculo de hash SHA1
        let mut sha1_context = Context::new(&SHA1_FOR_LEGACY_USE_ONLY);
        let mut stream = response.bytes_stream();
        let mut bytes_written = 0u64;

        let write_result: Result<(), ProtonError> = async {
            loop {
                match stream.try_next().await {
                    Ok(Some(chunk)) => {
                        sha1_context.update(&chunk);
                        file.write_all(&chunk).await?;
                        bytes_written += chunk.len() as u64;
                    }
                    Ok(None) => break,
                    Err(e) => return Err(ProtonError::RequestError(e)),
                }
            }
            file.flush().await?;
            Ok(())
        }
        .await;

        match write_result {
            Ok(()) => {
                // Verificar hash
                let actual_hash = hex::encode(sha1_context.finish());
                if actual_hash == expected_hash {
                    // Mover archivo temporal al destino final
                    match rename(&temp_file, &path).await {
                        Ok(()) => {
                            info!("File downloaded successfully: {:?}", path);
                            return Ok(());
                        }
                        Err(e) => {
                            error!("Failed to rename temp file: {}", e);
                            let _ = remove_file(&temp_file).await;
                            return Err(ProtonError::IoError(e));
                        }
                    }
                } else {
                    warn!(
                        "Hash mismatch on attempt {}: expected {}, got {}",
                        attempt, expected_hash, actual_hash
                    );
                }
            }
            Err(e) => {
                warn!("Write error on attempt {}: {}", attempt, e);
                if attempt == MAX_DOWNLOAD_ATTEMPTS {
                    // Limpiar archivo temporal antes de retornar error
                    let _ = remove_file(&temp_file).await;
                    return Err(e);
                }
            }
        }

        // Limpiar archivo temporal antes del siguiente intento
        if temp_file.exists() {
            if let Err(e) = remove_file(&temp_file).await {
                warn!("Failed to remove temp file: {}", e);
            }
        }

        // Opcional: delay exponencial entre intentos
        if attempt < MAX_DOWNLOAD_ATTEMPTS {
            let delay = Duration::from_millis(100 * (1 << (attempt - 1)));
            tokio::time::sleep(delay).await;
        }
    }

    Err(ProtonError::HashMismatch)
}

// Función auxiliar para verificar el hash de un archivo existente
async fn verify_file_hash(path: &PathBuf, expected_hash: &str) -> Result<bool, ProtonError> {
    let mut file = File::open(path).await.map_err(ProtonError::IoError)?;

    let mut sha1_context = Context::new(&SHA1_FOR_LEGACY_USE_ONLY);
    let mut buffer = [0u8; 8192]; // Buffer de 8KB para lectura eficiente

    loop {
        let bytes_read = file.read(&mut buffer).await.map_err(ProtonError::IoError)?;

        if bytes_read == 0 {
            break;
        }

        sha1_context.update(&buffer[..bytes_read]);
    }

    let actual_hash = hex::encode(sha1_context.finish());
    Ok(actual_hash == expected_hash)
}

pub async fn extract_native(jar_path: &PathBuf, destino: &PathBuf) -> Result<(), ProtonError> {
    // Abrir zip
    let reader = ZipFileReader::new(jar_path).await?;

    for i in 0..reader.file().entries().len() {
        let entry = &reader.file().entries()[i];
        let nombre = entry.filename().as_str()?;

        // Abrir reader para la entrada i
        let mut entry_reader = reader.reader_with_entry(i).await?;
        let mut contenido = Vec::with_capacity(entry.uncompressed_size() as usize);
        entry_reader.read_to_end_checked(&mut contenido).await?;

        if nombre.starts_with("META-INF/") {
            continue;
        }
        if nombre.ends_with("git") || nombre.ends_with("sha1") {
            continue;
        }
        let ruta_salida = destino.join(nombre);

        if let Some(p) = ruta_salida.parent() {
            create_dir_all(p).await?;
        }

        let mut archivo = File::create(&ruta_salida).await?;
        archivo.write_all(&contenido).await?;
    }

    Ok(())
}

pub fn get_os_name_runtime() -> &'static str {
    use os_info::Type;

    match os_info::get().os_type() {
        // Linux y distribuciones
        Type::Linux
        | Type::Ubuntu
        | Type::Debian
        | Type::Arch
        | Type::Manjaro
        | Type::Redhat
        | Type::Fedora
        | Type::Alpine
        | Type::OracleLinux
        | Type::EndeavourOS
        | Type::Pop
        | Type::Void
        | Type::NixOS => "linux",

        // macOS
        Type::Macos => "macos",

        // Windows
        Type::Windows => "windows",

        // Otros no soportados
        other => {
            println!("⚠️ OS no reconocido: {:?}", other);
            "unknown"
        }
    }
}
