extern crate image;
extern crate sha2;
extern crate rusqlite;
extern crate tiny_http;
extern crate regex;
extern crate handlebars;
extern crate rustc_serialize;
extern crate ascii;
extern crate chrono;

use std::sync::{Arc,RwLock};
use std::env;
use std::fs::{create_dir};
use std::path::{PathBuf};

use db::DataStore;
use file::GalleryScanner;

mod db;
mod file;
mod context;
mod web;
mod gallery;

fn main() {
    let file_dir = match env::home_dir() {
        Some(mut dir) => {
            dir.push(".hostimg");
            if !dir.exists() {
                create_dir(&dir).unwrap();
            }

            dir
        },
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

    let mut context = context::ServerContext {
        port: 1080,
        server_threads: 4,

        gallery_dir: gallery_dir,
        thumb_dir: thumb_dir,
        preview_dir: preview_dir,

        root_gallery: None,

        datastore: RwLock::new(store)
    };

    {
        let mut scanner = GalleryScanner::new(&mut context);
        if let Err(e) = scanner.scan() {
            println!("Scanning of file system failed: {:?}", e);
            return;
        }
    }

    println!("Running");

    let context = Arc::new(context);

    match web::WebServer::new(context.clone()) {
        Ok(mut server) => {
            if let Err(e) = server.register_action(Box::new(gallery::GalleryAction::new(context.clone()))) {
                println!("Failed to register GalleryAction: {:?}", e);
            }
            if let Err(e) = server.register_action(Box::new(gallery::ImageAction::new(context.clone()))) {
                println!("Failed to register ImageAction: {:?}", e);
            }

            server.run_webserver();
        },
        Err(e) => {
            println!("Server failed to start: {:?}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
