use anyhow::Context;
use tokio::sync::OnceCell;

use crate::{
    downloads::utils::get_launcher_manifest,
    repository::{
        launcher_manifest::{LauncherManifest, LauncherManifestVersion},
        manifest::Manifest,
    },
};

// TODO: Finish this feature

pub static LAUNCHER_MANIFEST: OnceCell<ManifestState> = OnceCell::const_new();

#[derive(Debug)]
pub struct ManifestState {
    pub launcher: LauncherManifest,
}

impl ManifestState {
    pub fn find_version(&self, version: impl Into<String>) -> Option<&LauncherManifestVersion> {
        let version = version.into();
        self.launcher.versions.iter().find(|v| v.id == version)
    }

    pub async fn get_version_manifest(
        &self,
        version: impl Into<String>,
    ) -> anyhow::Result<Manifest> {
        let url = &self
            .find_version(version)
            .context("cannot find such version")?
            .url;

        super::get(url).await
    }
}

pub async fn try_init() -> anyhow::Result<ManifestState> {
    Ok(ManifestState {
        launcher: get_launcher_manifest().await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn init_test() {
        let m = LAUNCHER_MANIFEST.get_or_try_init(try_init).await.unwrap();
        println!("{:?}", &m.launcher.versions[..5])
    }
}
