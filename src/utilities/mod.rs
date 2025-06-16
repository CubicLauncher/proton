use crate::errors::ProtonError;
use async_zip::tokio::read::fs::ZipFileReader;
use futures::TryStreamExt;
use hex;
use log::warn;
use once_cell::sync::Lazy;
use reqwest::Client;
use ring::digest::{Context, SHA1_FOR_LEGACY_USE_ONLY};
use std::path::Path;
use std::path::PathBuf;
use tokio::{
    fs::{File, create_dir_all, remove_file, rename},
    io::AsyncWriteExt,
};
use crate::types::NormalizedVersion;

pub static HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .user_agent("Cubic Proton/1.0")
        .build()
        .expect("Failed to build reqwest client")
});

const MAX_DOWNLOAD_ATTEMPTS: usize = 3;

pub async fn download_file(
    url: String,
    path: PathBuf,
    expected_hash: String,
) -> Result<(), ProtonError> {
    let temp_file = path.with_extension("tmp");
    for _attempt in 1..=MAX_DOWNLOAD_ATTEMPTS {
        let response = HTTP_CLIENT
            .get(&url)
            .send()
            .await
            .map_err(|e| ProtonError::RequestError(e))?;

        // Crea el directorio de destino si no existe
        if let Some(parent_dir) = path.parent() {
            create_dir_all(parent_dir).await?;
        }

        // Crea archivo de destino
        let mut file = File::create(&temp_file)
            .await
            .map_err(|e| ProtonError::IoError(e))?;

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

        if actual_hash == expected_hash {
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

pub async fn extract_native(
    jar_path: &Path,
    destino: &PathBuf,
) -> Result<(), ProtonError> {
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

        let ruta_salida = destino.join(nombre);
        println!("{}", ruta_salida.display());

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

// hace falta probar.
pub fn resolve_classpath(game_version: &NormalizedVersion) -> Result<Vec<String>, ProtonError> {
    
    let libs = game_version.libraries.iter().map(|lib| {
        let name = lib.name.clone();
        let path = format!("./lib/{}", name);
        Ok(path)
    }).collect::<Result<Vec<String>, ProtonError>>()?;
    Ok(libs)
}