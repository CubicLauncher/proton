use std::path::PathBuf;
use std::process::Command;

struct MinecraftLauncher {
    path: PathBuf
}

impl MinecraftLauncher {
    pub fn new(path: PathBuf) -> MinecraftLauncher {
        MinecraftLauncher { path }
    }
    pub fn launch_version(version: String) {}
}