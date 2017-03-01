use std::sync::Arc;
use std::path::PathBuf;

use file::ImageGallery;
use db::DataStore;

pub struct ServerContext {
    pub port: u16,
    pub server_threads: u32,

    pub gallery_dir: PathBuf,
    pub thumb_dir: PathBuf,
    pub preview_dir: PathBuf,

    pub root_gallery: Option<Arc<ImageGallery>>,

    pub datastore: DataStore
}
