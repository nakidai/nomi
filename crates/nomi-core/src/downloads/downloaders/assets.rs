use anyhow::Result;
use itertools::Itertools;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use tracing::info;

use crate::{
    downloads::{
        downloaders::file::FileDownloader,
        progress::ProgressSender,
        set::DownloadSet,
        traits::{DownloadResult, Downloadable, Downloader, DownloaderIO, DownloaderIOExt},
    },
    fs::write_json_config,
};

use super::DownloadQueue;

#[derive(Serialize, Deserialize, Debug)]
pub struct Assets {
    pub objects: HashMap<String, AssetInformation>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AssetInformation {
    pub hash: String,
    pub size: i64,
}

#[derive(Debug)]
pub struct AssetsDownloader {
    queue: DownloadQueue,
    assets: Assets,
    indexes: PathBuf,
    id: String,
}

#[derive(Debug)]
pub struct Chunk {
    ok: usize,
    err: usize,
    set: DownloadSet,
}

impl Chunk {
    pub fn new(set: DownloadSet) -> Self {
        Self { ok: 0, err: 0, set }
    }
}

#[async_trait::async_trait]
impl Downloader for Chunk {
    type Data = DownloadResult;

    fn total(&self) -> u32 {
        self.set.total()
    }

    async fn download(mut self: Box<Self>, sender: &dyn ProgressSender<Self::Data>) {
        let (helper_sender, mut helper_receiver) = tokio::sync::mpsc::channel(100);
        let downloader = self.set.with_helper(helper_sender);
        Box::new(downloader).download(sender).await;
        if let Some(result) = helper_receiver.recv().await {
            match result.0 {
                Ok(_) => self.ok += 1,
                Err(_) => self.err += 1,
            }
        }
        info!("Downloaded Chunk OK: {} ERR: {}", self.ok, self.err);
    }
}

impl AssetsDownloader {
    pub async fn new(url: String, id: String, objects: PathBuf, indexes: PathBuf) -> Result<Self> {
        let assets: Assets = Client::new().get(&url).send().await?.json().await?;

        let mut queue = DownloadQueue::new();

        assets
            .objects
            .iter()
            .collect_vec()
            .chunks(100)
            .map(|c| c.iter().map(|(_, v)| v).copied())
            .map(|chunk| {
                chunk
                    .filter_map(|asset| {
                        let path = objects.join(&asset.hash[0..2]).join(&asset.hash);
                        (!path.exists()).then_some(FileDownloader::new(
                            format!(
                                "https://resources.download.minecraft.net/{}/{}",
                                &asset.hash[0..2],
                                asset.hash
                            ),
                            path,
                        ))
                    })
                    .map::<Box<dyn Downloadable<Out = DownloadResult>>, _>(|downloader| {
                        Box::new(downloader)
                    })
                    .collect::<Vec<_>>()
            })
            .map(DownloadSet::from_vec_dyn)
            .map(Chunk::new)
            .for_each(|downloader| queue.add_downloader(downloader));

        Ok(Self {
            queue,
            assets,
            indexes,
            id,
        })
    }
}

impl<'a> DownloaderIOExt<'a> for AssetsDownloader {
    type IO = AssetsDownloaderIo<'a>;

    fn get_io(&'a self) -> AssetsDownloaderIo<'a> {
        AssetsDownloaderIo {
            assets: &self.assets,
            indexes: self.indexes.clone(),
            id: self.id.clone(),
        }
    }
}

pub struct AssetsDownloaderIo<'a> {
    assets: &'a Assets,
    indexes: PathBuf,
    id: String,
}

#[async_trait::async_trait]
impl DownloaderIO for AssetsDownloaderIo<'_> {
    async fn io(&self) -> anyhow::Result<()> {
        let path = self.indexes.join(format!("{}.json", self.id));
        write_json_config(&self.assets, path).await
    }
}

#[async_trait::async_trait]
impl Downloader for AssetsDownloader {
    type Data = DownloadResult;

    fn total(&self) -> u32 {
        self.queue.total()
    }

    #[tracing::instrument(skip_all)]
    async fn download(self: Box<Self>, sender: &dyn ProgressSender<Self::Data>) {
        Box::new(self.queue).download(sender).await;
    }
}
