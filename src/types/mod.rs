use crate::errors::ProtonError;
use crate::utilities::get_os_name_runtime;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

// URLs de los manifiestos oficiales de Mojang
pub const MOJANG_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const RESOURCES_BASE_URL: &str = "https://resources.download.minecraft.net/";

// Tipos de versión
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum VersionTypes {
    Snapshot,
    Release,
    #[serde(rename = "old_beta")]
    OldBeta,
    #[serde(rename = "old_alpha")]
    OldAlpha,
}

// Manifest principal de versiones
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangVersionManifest {
    pub latest: MojangLatestVersions,
    pub versions: Vec<MojangVersionInfo>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangLatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangVersionInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: VersionTypes,
    pub url: String,
    pub time: String,
    pub release_time: String,
    pub sha1: String,
}

// Detalles de una versión específica
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangVersionDetails {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: VersionTypes,
    pub main_class: String,
    pub minimum_launcher_version: u32,
    pub release_time: String,
    pub time: String,
    pub assets: String,
    pub asset_index: MojangAssetIndex,
    pub downloads: MojangDownloads,
    pub libraries: Vec<MojangLibrary>,
    pub logging: Option<MojangLogging>,
    pub arguments: Option<MojangArguments>,
    pub minecraft_arguments: Option<String>,
    pub java_version: Option<MojangJavaVersion>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangAssetIndex {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub total_size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangDownloads {
    pub client: MojangDownloadArtifact,
    pub client_mappings: Option<MojangDownloadArtifact>,
    pub server: Option<MojangDownloadArtifact>,
    pub server_mappings: Option<MojangDownloadArtifact>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangDownloadArtifact {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangLibrary {
    pub name: String,
    pub downloads: MojangLibraryDownloads,
    #[serde(default)]
    pub rules: Vec<MojangRule>,
    pub natives: Option<HashMap<String, String>>,
    pub extract: Option<MojangExtract>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangLibraryDownloads {
    pub artifact: Option<MojangArtifact>,
    #[serde(default)]
    pub classifiers: HashMap<String, MojangArtifact>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangArtifact {
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangRule {
    pub action: String,
    pub os: Option<MojangOSRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangOSRule {
    pub name: Option<String>,
    pub version: Option<String>,
    pub arch: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangExtract {
    pub exclude: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangLogging {
    pub client: MojangLoggerConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangLoggerConfig {
    pub argument: String,
    pub file: MojangLogFile,
    #[serde(rename = "type")]
    pub logger_type: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangLogFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangArguments {
    pub game: Vec<MojangArgumentValue>,
    pub jvm: Vec<MojangArgumentValue>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum MojangArgumentValue {
    Simple(String),
    Conditional {
        rules: Vec<MojangRule>,
        value: MojangConditionalValue,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum MojangConditionalValue {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MojangJavaVersion {
    pub component: String,
    pub major_version: u8,
}

// Estructuras normalizadas
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NormalizedVersion {
    pub id: String,
    pub release_time: String,
    pub java_version: u8,
    pub main_class: String,
    pub client_jar: Downloadable,
    pub server_jar: Option<Downloadable>,
    pub asset_index: AssetIndex,
    pub libraries: Vec<Library>,
    pub natives: Vec<NativeLibrary>,
    pub arguments: NormalizedArguments,
    pub requires_extraction: Vec<ExtractionHint>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Downloadable {
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AssetIndex {
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Library {
    pub name: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
    pub path: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NativeLibrary {
    pub name: String,
    pub classifier: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
    pub path: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ExtractionHint {
    pub path: String,
    pub requires_extraction: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NormalizedArguments {
    pub game: Vec<String>,
    pub jvm: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DownloadProgressInfo {
    pub name: String,
    pub version: Arc<String>,
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub current: usize,
    pub total: usize,
    pub info: DownloadProgressInfo,
    pub download_type: DownloadProgressType,
}
#[derive(Debug, Clone)]
pub enum DownloadProgressType {
    Library,
    Asset,
    Native,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VersionAssets {
    pub objects: HashMap<String, Asset>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Asset {
    pub hash: String,
    pub size: usize,
}

// Implementación de utilidad para VersionAssets
impl VersionAssets {
    /// Obtiene todos los assets como vector de tuplas (ruta, asset)
    pub fn as_vec(mut self) -> Vec<(String, Asset)> {
        self.objects.drain().collect()
    }

    /// Obtiene un asset específico por ruta
    pub fn get_asset(&self, path: &str) -> Option<&Asset> {
        self.objects.get(path)
    }

    /// Número total de assets
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Verifica si no hay assets
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

// Implementación de conversión de Mojang a Normalized
impl TryFrom<MojangVersionDetails> for NormalizedVersion {
    type Error = ProtonError;

    fn try_from(mojang_version: MojangVersionDetails) -> Result<Self, Self::Error> {
        let assets = mojang_version.asset_index;
        let downloads = mojang_version.downloads;

        // Convertir librerías
        let mut libraries = Vec::new();
        let mut natives = Vec::new();
        let mut requires_extraction = Vec::new();

        for lib in mojang_version.libraries {
            // Verificar reglas de la librería
            if !library_applies(&lib) {
                continue;
            }

            if let Some(artifact) = lib.downloads.artifact {
                libraries.push(Library {
                    name: lib.name.clone(),
                    url: artifact.url,
                    sha1: artifact.sha1,
                    size: artifact.size,
                    path: artifact.path,
                });
            }

            // Procesar nativos
            if let Some(natives_map) = lib.natives {
                if let Some(classifier) = natives_map.get(get_os_name_runtime()) {
                    if let Some(native_artifact) = lib.downloads.classifiers.get(classifier) {
                        natives.push(NativeLibrary {
                            name: lib.name,
                            classifier: classifier.clone(),
                            url: native_artifact.url.clone(),
                            sha1: native_artifact.sha1.clone(),
                            size: native_artifact.size,
                            path: native_artifact.path.clone(),
                        });

                        requires_extraction.push(ExtractionHint {
                            path: native_artifact.path.clone(),
                            requires_extraction: true,
                        });
                    }
                }
            }
        }

        // Convertir argumentos
        let arguments = match (mojang_version.arguments, mojang_version.minecraft_arguments) {
            (Some(args), _) => normalize_arguments(args),
            (None, Some(legacy_args)) => parse_legacy_arguments(legacy_args),
            (None, None) => NormalizedArguments {
                game: Vec::new(),
                jvm: Vec::new(),
            },
        };

        Ok(NormalizedVersion {
            id: mojang_version.id,
            release_time: mojang_version.release_time,
            java_version: mojang_version
                .java_version
                .as_ref()
                .map_or(8, |v| v.major_version),
            main_class: mojang_version.main_class,
            client_jar: Downloadable {
                url: downloads.client.url,
                sha1: downloads.client.sha1,
                size: downloads.client.size,
            },
            server_jar: downloads.server.map(|s| Downloadable {
                url: s.url,
                sha1: s.sha1,
                size: s.size,
            }),
            asset_index: AssetIndex {
                id: assets.id,
                url: assets.url,
                sha1: assets.sha1,
                size: assets.size,
            },
            libraries,
            natives,
            arguments,
            requires_extraction,
        })
    }
}

// Funciones helper
fn library_applies(lib: &MojangLibrary) -> bool {
    if lib.rules.is_empty() {
        return true;
    }

    let os_name = get_os_name_runtime();
    let mut allow = false;

    for rule in &lib.rules {
        let applies = match &rule.os {
            Some(os_rule) => {
                let name_match = os_rule.name.as_ref().map_or(true, |n| n == os_name);
                name_match
            }
            None => true,
        };

        match rule.action.as_str() {
            "allow" => {
                if applies {
                    allow = true;
                }
            }
            "disallow" => {
                if applies {
                    return false;
                }
            }
            _ => {}
        }
    }

    allow
}

fn normalize_arguments(args: MojangArguments) -> NormalizedArguments {
    let game = flatten_arguments(args.game);
    let jvm = flatten_arguments(args.jvm);

    NormalizedArguments { game, jvm }
}

fn flatten_arguments(args: Vec<MojangArgumentValue>) -> Vec<String> {
    let mut result = Vec::new();
    let os_name = get_os_name_runtime();

    for arg in args {
        match arg {
            MojangArgumentValue::Simple(s) => {
                result.push(s);
            }
            MojangArgumentValue::Conditional { rules, value } => {
                if rule_set_applies(&rules, os_name) {
                    match value {
                        MojangConditionalValue::Single(s) => result.push(s),
                        MojangConditionalValue::Multiple(v) => result.extend(v),
                    }
                }
            }
        }
    }

    result
}

fn rule_set_applies(rules: &[MojangRule], os_name: &str) -> bool {
    if rules.is_empty() {
        return true;
    }

    let mut allow = false;

    for rule in rules {
        let applies = match &rule.os {
            Some(os_rule) => os_rule.name.as_ref().map_or(true, |n| n == os_name),
            None => true,
        };

        match rule.action.as_str() {
            "allow" => {
                if applies {
                    allow = true;
                }
            }
            "disallow" => {
                if applies {
                    return false;
                }
            }
            _ => {}
        }
    }

    allow
}

fn parse_legacy_arguments(args: String) -> NormalizedArguments {
    let mut game_args = Vec::new();
    let mut jvm_args = Vec::new();

    // Parsear argumentos del juego
    for arg in args.split_whitespace() {
        game_args.push(arg.to_string());
    }

    // Argumentos JVM estándar para versiones antiguas
    jvm_args.push("-Djava.library.path=${natives_directory}".to_string());
    jvm_args.push("-cp".to_string());
    jvm_args.push("${classpath}".to_string());

    NormalizedArguments {
        game: game_args,
        jvm: jvm_args,
    }
}
