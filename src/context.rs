use std::sync::{RwLock,Arc};
use std::path::PathBuf;
use std::marker::{Sync,Send};

use file::ImageGallery;
use db::DataStore;

pub struct ServerContext {
    pub port: u16,
    pub server_threads: usize,

    pub gallery_dir: PathBuf,
    pub thumb_dir: PathBuf,
    pub preview_dir: PathBuf,

    pub root_gallery: Option<Arc<ImageGallery>>,

    pub datastore: RwLock<DataStore>
}

unsafe impl Sync for ServerContext {
}

unsafe impl Send for ServerContext {
}
