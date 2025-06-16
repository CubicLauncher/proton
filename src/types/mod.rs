use std::collections::HashMap;
use serde::Deserialize;

pub const MANIFEST_URL: &str = "https://manifest.cubicmc.me/manifest";
pub const VERSION_INDEX_URL: &str = "https://manifest.cubicmc.me/version";
pub const RESOURCES_BASE_URL: &str = "https://resources.download.minecraft.net/";

#[derive(Debug, Deserialize, Clone)]
pub struct MinecraftVersion {
    pub id: String,
    pub sha1: String,
    pub release_time: String,
    pub url: String,
    #[serde(rename = "type")]
    pub version_type: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct VersionManifest {
    pub latest_release: String,
    pub latest_snapshot: String,
    pub versions: Vec<MinecraftVersion>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NormalizedVersion {
    pub id: String,
    pub release_time: String,
    pub java_version: u8,
    pub client_jar: Downloadable,
    pub server_jar: Option<Downloadable>,
    pub asset_index: AssetIndex,
    pub libraries: Vec<Library>,
    pub natives: Vec<NativeLibrary>,
    pub arguments: NormalizedArguments,
    pub requires_extraction: Vec<ExtractionHint>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Downloadable {
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AssetIndex {
    pub id: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Library {
    pub name: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
    pub path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NativeLibrary {
    pub name: String,
    pub classifier: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
    pub path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExtractionHint {
    pub path: String,
    pub requires_extraction: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NormalizedArguments {
    pub game: Vec<String>,
    pub jvm: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub current: usize,
    pub total: usize,
    pub name: Option<String>, // nombre del archivo o asset opcional
}

// Fixed: Use HashMap instead of Vec<(String, Asset)>
#[derive(Debug, Deserialize, Clone)]
pub struct VersionAssets {
    pub objects: HashMap<String, Asset>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Asset {
    pub hash: String,
    pub size: usize,
}

// Helper implementation for VersionAssets to make it easier to work with
impl VersionAssets {
    /// Get all assets as a vector of (path, asset) tuples
    pub fn as_vec(&self) -> Vec<(&String, &Asset)> {
        self.objects.iter().collect()
    }
    
    /// Get a specific asset by path
    pub fn get_asset(&self, path: &str) -> Option<&Asset> {
        self.objects.get(path)
    }
    
    /// Get the total number of assets
    pub fn len(&self) -> usize {
        self.objects.len()
    }
    
    /// Check if assets is empty
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}