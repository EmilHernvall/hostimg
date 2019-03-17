use std::env;
use std::fs::create_dir;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::db::DataStore;
use crate::file::GalleryScanner;

mod context;
mod db;
mod file;
mod gallery;
mod web;

fn main() {
    let file_dir = match env::home_dir() {
        Some(mut dir) => {
            dir.push(".hostimg");
            if !dir.exists() {
                create_dir(&dir).unwrap();
            }

            dir
        }
        None => {
            println!("Couldn't figure out home directory.");
            return;
        }
    };

    println!("Storing files in {:?}", file_dir);

    let store = match DataStore::new(&file_dir) {
        Ok(x) => x,
        Err(e) => {
            println!("Failed to create store: {:?}", e);
            return;
        }
    };

    let mut thumb_dir = file_dir.clone();
    thumb_dir.push("thumb");

    if !thumb_dir.exists() {
        create_dir(&thumb_dir).unwrap();
    }

    let mut preview_dir = file_dir.clone();
    preview_dir.push("preview");

    if !preview_dir.exists() {
        create_dir(&preview_dir).unwrap();
    }

    let gallery_dir = match env::args().nth(1) {
        Some(x) => PathBuf::from(x),
        None => {
            println!("Specify a directory to scan");
            return;
        }
    };

    let context = context::ServerContext {
        port: 1080,
        server_threads: 4,

        gallery_dir: gallery_dir,
        thumb_dir: thumb_dir,
        preview_dir: preview_dir,

        root_gallery: Arc::new(RwLock::new(None)),

        datastore: store,
    };

    let mut scanner = GalleryScanner::new(context.clone());

    if let Err(e) = scanner.scan() {
        println!("Scanning of file system failed: {:?}", e);
        return;
    }

    scanner.process_images();
    println!("Running");

    web::run_server(context.clone());

    let _ = scanner.monitor();
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
