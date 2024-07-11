use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{mpsc::Sender, Arc},
};

use egui_task_manager::{Progress, TaskProgressShared};
use itertools::Itertools;
use nomi_core::{
    calculate_sha1,
    downloads::{progress::MappedSender, traits::Downloader, DownloadSet, FileDownloader},
    fs::read_toml_config,
};
use nomi_modding::{
    modrinth::{
        project::ProjectId,
        version::{Dependency, ProjectVersionsData, Version, VersionId},
    },
    Query,
};
use serde::{Deserialize, Serialize};
use tokio::{fs::File, io::AsyncWriteExt};

use crate::{
    progress::UnitProgress, DOT_NOMI_MODS_STASH_DIR, MINECRAFT_MODS_DIRECTORY,
    NOMI_LOADED_LOCK_FILE, NOMI_LOADED_LOCK_FILE_NAME,
};

#[derive(Serialize, Deserialize, Default, PartialEq, Eq, Hash, Debug)]
pub struct ModsConfig {
    pub mods: Vec<Mod>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug)]
pub struct Mod {
    pub project_id: ProjectId,
    pub name: String,
    pub version_id: VersionId,
    pub is_installed: bool,
    pub files: Vec<ModFile>,
    pub dependencies: Vec<Dependency>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug)]
pub struct ModFile {
    pub sha1: String,
    pub url: String,
    pub filename: String,
}

pub struct SimpleDependency {
    pub name: String,
    pub versions: Vec<Arc<Version>>,
    pub is_required: bool,
}

pub async fn proceed_deps(
    dist: &mut Vec<SimpleDependency>,
    version: Arc<Version>,
    game_version: String,
    loader: String,
) -> anyhow::Result<()> {
    for dep in &version.dependencies {
        let query = Query::new(
            ProjectVersionsData::builder()
                .id_or_slug(dep.project_id.clone())
                .game_versions(vec![game_version.clone()])
                .loaders(vec![loader.clone()])
                .build(),
        );

        let data = query.query().await?;

        let versions = data.into_iter().map(Arc::new).collect_vec();

        dist.push(SimpleDependency {
            name: versions
                .first()
                .map_or(format!("Dependency. {:?}", &dep.project_id), |v| {
                    v.name.clone()
                }),
            versions: versions.clone(),
            is_required: dep
                .dependency_type
                .as_ref()
                .is_some_and(|d| d == "required")
                || dep.dependency_type.is_none(),
        });
    }

    Ok(())
}

pub async fn download_mods(
    progress: TaskProgressShared,
    dir: PathBuf,
    versions: Vec<Arc<Version>>,
) -> Vec<Mod> {
    let _ = progress.set_total(
        versions
            .iter()
            .map(|v| {
                v.files
                    .iter()
                    .filter(|f| f.primary)
                    .collect::<Vec<_>>()
                    .len() as u32
            })
            .sum(),
    );

    let mut mods = Vec::new();
    for version in versions {
        let mod_value = download_mod(progress.sender(), dir.clone(), version).await;
        mods.push(mod_value);
    }
    mods
}

pub async fn download_mod(
    sender: Sender<Box<dyn Progress>>,
    dir: PathBuf,
    version: Arc<Version>,
) -> Mod {
    let mut set = DownloadSet::new();

    let mut downloaded_files = Vec::new();

    // We do not download any dependencies. Just the mod.
    for file in version.files.iter().filter(|f| f.primary) {
        if tokio::fs::read_to_string(dir.join(&file.filename))
            .await
            .is_ok_and(|s| calculate_sha1(s) == file.hashes.sha1)
        {
            let _ = sender.send(Box::new(UnitProgress));
            continue;
        }

        downloaded_files.push(ModFile {
            sha1: file.hashes.sha1.clone(),
            url: file.url.clone(),
            filename: file.filename.clone(),
        });

        let downloader = FileDownloader::new(file.url.clone(), dir.join(&file.filename))
            .with_sha1(file.hashes.sha1.clone());
        set.add(Box::new(downloader));
    }

    let sender = MappedSender::new_progress_mapper(Box::new(sender));

    Box::new(set).download(&sender).await;

    Mod {
        name: version.name.clone(),
        version_id: version.id.clone(),
        is_installed: true,
        files: downloaded_files,
        dependencies: version.dependencies.clone(),
        project_id: version.project_id.clone(),
    }
}

#[derive(Serialize, Deserialize)]
pub struct CurrentlyLoaded {
    id: usize,
}

impl CurrentlyLoaded {
    pub async fn write_with_comment(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let mut file = File::create(path.as_ref()).await?;

        file.write_all(b"# This file is automatically generated by Nomi.\n# It is not intended for manual editing.\n").await?;
        file.write_all(toml::to_string_pretty(&self)?.as_bytes())
            .await?;

        file.flush().await?;

        Ok(())
    }
}

/// Load profile's mods by creating hard links.
pub async fn load_mods(profile_id: usize) -> anyhow::Result<()> {
    async fn make_link(source: &Path, file_name: &OsStr) -> anyhow::Result<()> {
        let dst = PathBuf::from(MINECRAFT_MODS_DIRECTORY).join(file_name);
        tokio::fs::hard_link(source, dst)
            .await
            .map_err(|e| e.into())
    }

    if !Path::new(NOMI_LOADED_LOCK_FILE).exists() {
        CurrentlyLoaded { id: profile_id }
            .write_with_comment(NOMI_LOADED_LOCK_FILE)
            .await?
    }

    let mut loaded = read_toml_config::<CurrentlyLoaded>(NOMI_LOADED_LOCK_FILE).await?;

    if loaded.id == profile_id {
        let path = PathBuf::from(DOT_NOMI_MODS_STASH_DIR).join(format!("{profile_id}"));
        let mut dir = tokio::fs::read_dir(path).await?;

        let target_dir = PathBuf::from(MINECRAFT_MODS_DIRECTORY)
            .read_dir()?
            .filter_map(|r| r.ok())
            .map(|e| e.file_name())
            .collect::<Vec<_>>();

        while let Ok(Some(entry)) = dir.next_entry().await {
            if target_dir.contains(&entry.file_name()) {
                continue;
            }

            let path = entry.path();

            let Some(file_name) = path.file_name() else {
                continue;
            };

            make_link(&path, file_name).await?;
        }

        return Ok(());
    }

    let mut dir = tokio::fs::read_dir(MINECRAFT_MODS_DIRECTORY).await?;
    while let Ok(Some(entry)) = dir.next_entry().await {
        if entry.file_name() == NOMI_LOADED_LOCK_FILE_NAME {
            continue;
        }

        tokio::fs::remove_file(entry.path()).await?;
    }

    let mut dir =
        tokio::fs::read_dir(PathBuf::from(DOT_NOMI_MODS_STASH_DIR).join(format!("{profile_id}")))
            .await?;

    while let Ok(Some(entry)) = dir.next_entry().await {
        let path = entry.path();

        let Some(file_name) = path.file_name() else {
            continue;
        };

        make_link(&path, file_name).await?;
    }

    loaded.id = profile_id;

    loaded.write_with_comment(NOMI_LOADED_LOCK_FILE).await?;

    Ok(())
}
