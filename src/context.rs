use std::path::PathBuf;
use std::result::Result;
use std::sync::{Arc, RwLock};

use crate::db::DataStore;
use crate::file::ImageGallery;

#[derive(Debug)]
pub enum ContextError {
    GalleryAccessError,
    GalleryNotSetError,
}

#[derive(Clone)]
pub struct ServerContext {
    pub port: u16,
    pub server_threads: usize,

    pub gallery_dir: PathBuf,
    pub thumb_dir: PathBuf,
    pub preview_dir: PathBuf,

    pub root_gallery: Arc<RwLock<Option<Arc<ImageGallery>>>>,

    pub datastore: DataStore,
}

impl ServerContext {
    pub fn set_root_gallery(&self, gallery: Arc<ImageGallery>) -> Result<(), ContextError> {
        let mut root_gallery = self
            .root_gallery
            .write()
            .or(Err(ContextError::GalleryAccessError))?;

        *root_gallery = Some(gallery);

        Ok(())
    }

    pub fn get_root_gallery(&self) -> Result<Arc<ImageGallery>, ContextError> {
        match self.root_gallery.read() {
            Ok(ref r) => match **r {
                Some(ref x) => Ok(x.clone()),
                None => Err(ContextError::GalleryNotSetError),
            },
            Err(_) => Err(ContextError::GalleryAccessError),
        }
    }
}
