use std::sync::{RwLock,Arc};
use std::path::PathBuf;
use std::marker::{Sync,Send};
use std::cell::RefCell;
use std::result::Result;

use file::ImageGallery;
use db::DataStore;

pub enum ContextError {
    GalleryAccessError,
    GalleryNotSetError
}

pub struct ServerContext {
    pub port: u16,
    pub server_threads: usize,

    pub gallery_dir: PathBuf,
    pub thumb_dir: PathBuf,
    pub preview_dir: PathBuf,

    pub root_gallery: RefCell<Option<Arc<ImageGallery>>>,

    pub datastore: RwLock<DataStore>
}

impl ServerContext {
    pub fn set_root_gallery(&self, gallery: Arc<ImageGallery>) -> Result<(), ContextError> {
        let mut root_gallery = self.root_gallery.try_borrow_mut()
            .or(Err(ContextError::GalleryAccessError))?;
        *root_gallery = Some(gallery);

        Ok(())
    }

    pub fn get_root_gallery(&self) -> Result<Arc<ImageGallery>, ContextError> {
        match self.root_gallery.try_borrow() {
            Ok(ref r) => {
                match **r {
                    Some(ref x) => Ok(x.clone()),
                    None => Err(ContextError::GalleryNotSetError)
                }
            },
            Err(_) => Err(ContextError::GalleryAccessError)
        }
    }
}

unsafe impl Sync for ServerContext {
}

unsafe impl Send for ServerContext {
}
