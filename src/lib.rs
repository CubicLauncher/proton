mod downloaders;
mod errors;
mod manifest;
mod types;
mod utilities;

pub use downloaders::MinecraftDownloader;
pub use errors::ProtonError;
pub use manifest::resolve_version_data;
pub use types::{DownloadProgress, DownloadProgressType};
#[cfg(test)]
mod tests {
    // #[test]
    // fn it_works() {
    //     let result = add(2, 2);
    //     assert_eq!(result, 4);
    // }
}
