# Proton
[![dependency status](https://deps.rs/repo/github/cubiclauncher/proton/status.svg)](https://deps.rs/repo/github/cubiclauncher/proton)

Una librería de Rust de alto rendimiento para descargar versiones de Minecraft de forma rápida y eficiente.

## Características

- **Descarga concurrente**: Hasta 128 descargas simultáneas para máxima velocidad
- **Descarga completa**: Cliente, librerías, assets y nativos en una sola operación
- **Verificación de integridad**: Verificación SHA1 automática de todos los archivos
- **Multiplataforma**: Soporte nativo para Windows, macOS y Linux
- **Seguimiento de progreso**: Callbacks en tiempo real del progreso de descarga
- **Gestión automática**: Extracción automática de nativos y limpieza de archivos temporales
- **Todas las versiones**: Soporte para releases, snapshots, betas y alfas

## Instalación

Agrega Proton a tu `Cargo.toml`:

```toml
[dependencies]
proton = { git = "https://github.com/CubicLauncher/proton.git" }
```

## Uso Básico

### Descarga Simple

```rust
use proton::{MinecraftDownloader, resolve_version_data};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Resolver datos de la versión
    let version_data = resolve_version_data("1.21.8".to_string()).await?;

    // Crear el descargador
    let mut downloader = MinecraftDownloader::new(
        PathBuf::from("./minecraft"),
        version_data,
    );

    // Descargar todo
    downloader.download_all(None).await?;

    println!("Descarga completada");
    Ok(())
}
```

### Con Seguimiento de Progreso

```rust
use proton::{MinecraftDownloader, resolve_version_data, DownloadProgress, DownloadProgressType};
use std::path::PathBuf;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let version_data = resolve_version_data("1.21.8".to_string()).await?;

    let mut downloader = MinecraftDownloader::new(
        PathBuf::from("./minecraft"),
        version_data,
    );

    // Canal para recibir actualizaciones de progreso
    let (tx, mut rx) = mpsc::channel(100);

    // Iniciar descarga en segundo plano
    let download_handle = tokio::spawn(async move {
        downloader.download_all(Some(tx)).await
    });

    // Escuchar actualizaciones de progreso
    while let Some(progress) = rx.recv().await {
        match progress.download_type {
            DownloadProgressType::Client => println!("Descargando cliente: {}/{}", progress.current, progress.total),
            DownloadProgressType::Library => println!("Descargando librería: {}/{}", progress.current, progress.total),
            DownloadProgressType::Asset => println!("Descargando asset: {}/{}", progress.current, progress.total),
            DownloadProgressType::Native => println!("Descargando nativo: {}/{}", progress.current, progress.total),
        }
    }

    // Esperar a que termine la descarga
    download_handle.await??;
    println!("Descarga completada");
    Ok(())
}
```

### Descarga de Versiones Específicas

```rust
use proton::{MinecraftDownloader, resolve_version_data};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Versiones disponibles
    let versions = vec![
        "1.21.8",      // Última release
        "24w14a",      // Snapshot
        "b1.7.3",      // Beta antigua
        "a1.2.6",      // Alpha antigua
    ];

    for version_id in versions {
        println!("Descargando versión: {}", version_id);

        match resolve_version_data(version_id.to_string()).await {
            Ok(version_data) => {
                let mut downloader = MinecraftDownloader::new(
                    PathBuf::from(format!("./minecraft/{}", version_id)),
                    version_data,
                );

                downloader.download_all(None).await?;
                println!("Versión {} descargada exitosamente", version_id);
            }
            Err(e) => println!("Error descargando {}: {}", version_id, e),
        }
    }

    Ok(())
}
```

## Estructura de Archivos

Proton organiza los archivos descargados de la siguiente manera:

```
minecraft/
├── versions/
│   └── 1.21.8/
│       └── 1.21.8.jar          # Cliente de Minecraft
├── libraries/
│   ├── com/
│   ├── org/
│   └── ...                     # Todas las librerías Java
├── assets/
│   └── objects/
│       ├── 00/
│       ├── 01/
│       └── ...                 # Assets organizados por hash
└── natives/
    └── 1.21.8/
        ├── lwjgl.dll           # Nativos extraídos
        └── ...
```

## API de Referencia

### `MinecraftDownloader`

La estructura principal para descargar versiones de Minecraft.

#### Métodos

- `new(game_path: PathBuf, game_version: NormalizedVersion) -> Self`
  - Crea un nuevo descargador para la versión especificada

- `download_all(progress_tx: Option<Sender<DownloadProgress>>) -> Result<(), ProtonError>`
  - Descarga todos los componentes de la versión de forma concurrente

### `resolve_version_data(version_id: String) -> Result<NormalizedVersion, ProtonError>`

Resuelve los metadatos de una versión específica de Minecraft.

### Tipos de Progreso

```rust
pub enum DownloadProgressType {
    Library,    // Librerías Java
    Asset,      // Assets del juego (texturas, sonidos, etc.)
    Native,     // Librerías nativas del sistema
    Client,     // Cliente JAR de Minecraft
}
```

## Rendimiento

Proton está optimizado para máxima velocidad:

- **128 descargas concurrentes** por defecto
- **Verificación SHA1** en paralelo
- **Extracción automática** de nativos
- **Gestión eficiente de memoria** con streams
- **Reintentos automáticos** en caso de fallo

## Ejemplos de Rendimiento

| Versión | Archivos | Tamaño | Tiempo Estimado |
|---------|----------|--------|-----------------|
| 1.21.8  | ~2,500   | ~200MB | ~30 segundos    |
| 1.20.4  | ~2,300   | ~180MB | ~25 segundos    |
| 1.19.4  | ~2,100   | ~160MB | ~20 segundos    |

*Tiempos estimados en conexión de 100 Mbps*

## Manejo de Errores

Proton proporciona errores detallados para facilitar el debugging:

```rust
use proton::errors::ProtonError;

match downloader.download_all(None).await {
    Ok(()) => println!("Descarga exitosa"),
    Err(ProtonError::VersionNotFound(version)) => {
        println!("Versión {} no encontrada", version);
    }
    Err(ProtonError::DownloadFailed(url, error)) => {
        println!("Error descargando {}: {}", url, error);
    }
    Err(ProtonError::HashMismatch(expected, actual)) => {
        println!("Hash incorrecto: esperado {}, obtenido {}", expected, actual);
    }
    Err(e) => println!("Error inesperado: {}", e),
}
```

## Dependencias

- `tokio` - Runtime asíncrono
- `reqwest` - Cliente HTTP
- `async-zip` - Extracción de archivos ZIP
- `serde` - Serialización/deserialización JSON
- `rayon` - Paralelización
- `ring` - Verificación SHA1

## Licencia

Este proyecto está bajo la licencia GPL-2.0. Ver el archivo `LICENSE` para más detalles.

## Contribuir

Las contribuciones son bienvenidas. Por favor:

1. Fork el repositorio
2. Crea una rama para tu feature (`git checkout -b username/nueva-funcionalidad`)
3. Commit tus cambios (`git commit -am 'Agregar nueva funcionalidad'`)
4. Push a la rama (`git push origin feature/nueva-funcionalidad`)
5. Abre un Pull Request
