use nomi_core::{
    downloads::traits::Downloader,
    game_paths::GamePaths,
    instance::{Instance, InstanceBuilder},
    loaders::vanilla::Vanilla,
};
use tracing::Level;

#[tokio::test]
async fn download_test() {
    let subscriber = tracing_subscriber::fmt()
        .pretty()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let (tx, _) = tokio::sync::mpsc::channel(100);

    let game_paths = GamePaths {
        game: "./minecraft".into(),
        assets: "./minecraft/assets".into(),
        version: ("./minecraft/versions/1.18.2".into()),
        libraries: "./minecraft/libraries".into(),
    };

    let instance = Instance::builder()
        .version("1.18.2".into())
        .instance(Box::new(
            Vanilla::new("1.18.2", game_paths.clone()).await.unwrap(),
        ))
        .game_paths(game_paths)
        .name("1.18.2-test".into())
        .build();

    Box::new(instance.assets().await.unwrap())
        .download(&tx)
        .await;

    let version = instance.instance();
    {
        let io = version.get_io_dyn();
        io.io().await.unwrap();
    }

    version.download(&tx).await;
}
