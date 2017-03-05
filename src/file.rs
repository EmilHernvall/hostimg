extern crate sha2;
extern crate image;

use std::ascii::AsciiExt;
use std::cmp::{PartialOrd,Ordering};
use std::fs::{read_dir, File};
use std::io::Error as IoError;
use std::io::Result as IoResult;
use std::io::{Read, BufReader, ErrorKind};
use std::path::{Path,PathBuf};
use std::sync::mpsc::channel;
use std::sync::{RwLock, Arc};
use std::time::Duration;
use std::collections::BTreeSet;

use notify::{Watcher, RecursiveMode, watcher, DebouncedEvent};
use image::{GenericImage, DynamicImage, ImageResult};
use sha2::Digest;

use db::ImageInfo;
use context::ServerContext;

pub fn open_image(file: &Path) -> ImageResult<DynamicImage> {
    let file_obj = File::open(&file)?;
    let reader = BufReader::new(file_obj);
    image::load(reader, image::ImageFormat::JPEG)
}

pub fn hash_file(file: &Path) -> IoResult<String> {
    let mut file_obj = File::open(file)?;

    let mut buffer = [0;4096];
    let mut hasher = sha2::Sha256::new();
    while let Ok(bytes_read) = file_obj.read(&mut buffer) {
        if bytes_read == 0 {
            break;
        }
        hasher.input(&buffer[0..bytes_read]);
    }

    let bin_hash = hasher.result();

    let mut hex_hash = String::new();
    for b in bin_hash {
        hex_hash.push_str(&format!("{:X}", b));
    }

    Ok(hex_hash)
}

pub fn is_image(file: &Path) -> bool {
    file.extension()
        .and_then(|x| x.to_str())
        .map(|s| s.to_ascii_lowercase() == "jpg")
        .unwrap_or(false)
}

pub struct GalleryScanner {
    context: Arc<ServerContext>
}

impl GalleryScanner {
    pub fn new(context: Arc<ServerContext>) -> GalleryScanner {
        GalleryScanner {
            context: context
        }
    }

    pub fn scan(&mut self) -> IoResult<()> {
        let gallery_dir = &self.context.gallery_dir.clone();

        let gallery = Arc::new(self.scan_recursive(gallery_dir,
                                                   &is_image)?);

        self.context.set_root_gallery(gallery)
            .or(build_io_error("Failed to set root gallery"))?;

        Ok(())
    }

    fn scan_recursive<F>(&mut self,
                         dir: &PathBuf,
                         accept: &F) -> IoResult<ImageGallery>
        where F: Fn(&Path) -> bool {

        let mut images = BTreeSet::new();
        let mut galleries = BTreeSet::new();
        for entry in read_dir(dir)?.filter_map(|x| x.ok()) {
            let filetype = entry.file_type()?;
            let p = entry.path();
            if filetype.is_dir() {
                match self.scan_recursive(&p, accept) {
                    Ok(gallery) => {
                        galleries.insert(Arc::new(gallery));
                    },
                    Err(e) => println!("Failed to add gallery {:?}", e)
                }
            } else if filetype.is_file() && accept(&p) {
                match self.add_file(&p) {
                    Ok(info) => {
                        images.insert(Arc::new(info));
                    },
                    Err(e) => println!("Failed to add image {:?}", e)
                }
            }
        }

        let local_path = dir.strip_prefix(&self.context.gallery_dir)
            .or(build_io_error("Failed to strip directory prefix"))?
            .to_path_buf();

        Ok(ImageGallery {
            path: local_path,
            sub_galleries: galleries,
            images: images
        })
    }

    fn add_file(&mut self, file: &PathBuf) -> IoResult<ImageInfo> {
        let file_name = match file.to_str() {
            Some(x) => x,
            None => return build_io_error("Failed to retrieve filename")
        };

        let query_result = match self.context.datastore.read() {
            Ok(datastore) => datastore.find_image_by_name(file_name.to_string())?,
            Err(_) => return build_io_error("Failed to acquire read lock for database")
        };

        if let Some(info) = query_result {
            println!("Found {}", &info.name);

            Ok(info)
        } else {
            println!("Adding {}", file_name);
            let image_file = ImageFile::build_from_path(file)?;

            if let Err(e) = image_file.scale_and_save(2048, 2048, &self.context.preview_dir) {
                println!("Failed to save preview for image {}: {:?}", file_name, e);
            }

            if let Err(e) = image_file.scale_and_save(256, 256, &self.context.thumb_dir) {
                println!("Failed to save thumb for image {}: {:?}", file_name, e);
            }

            let info = match image_file.build_info() {
                Some(info) => info,
                None => return build_io_error(format!("Well, this is rather inexplicable. Image: {}", file_name).as_str())
            };

            match self.context.datastore.write() {
                Ok(mut datastore) => datastore.save_image(&info)?,
                Err(_) => return build_io_error("Failed to acquire read lock for database")
            };

            return Ok(info);
        }
    }

    fn handle_update(&mut self, event: DebouncedEvent) -> IoResult<()> {
        let root_gallery = self.context.get_root_gallery()
            .or(build_io_error("Failed to get root gallery"))?;

        match event {
            DebouncedEvent::Create(path) => {
                if !is_image(&path) {
                    return Ok(());
                }

                let parent = match path.parent() {
                    Some(x) => x.to_path_buf(),
                    None => return build_io_error("Path has no parent")
                };

                let gallery = root_gallery.find_gallery_from_name(&parent);

                let info = self.add_file(&path).or(build_io_error("Failed to add file"));

                //gallery.
            },
            DebouncedEvent::Write(_) => {
            },
            DebouncedEvent::Remove(_) => {
            },
            DebouncedEvent::Rename(_, _) => {
            },
            DebouncedEvent::Rescan => {
            },
            _ => {
            }
        }

        Ok(())
    }

    pub fn monitor(mut self) -> IoResult<()> {
        let (tx, rx) = channel();

        let mut watcher = watcher(tx, Duration::from_secs(10))
            .or(build_io_error("Failed to create watcher"))?;
        watcher.watch(&self.context.gallery_dir, RecursiveMode::Recursive)
            .or(build_io_error("Failed to register watch"))?;

        loop {
            match rx.recv()
                .or(build_io_error("Failed to read event"))
                .and_then(|event| self.handle_update(event)) {

                Ok(_) => {},
                Err(e) => println!("Watch error: {:?}", e)
            }
        }
    }
}

fn build_io_error<T>(message: &str) -> IoResult<T> {
    Err(IoError::new(ErrorKind::Other, message))
}

pub struct ImageGallery {
    pub path: PathBuf,
    pub sub_galleries: BTreeSet<Arc<ImageGallery>>,
    pub images: BTreeSet<Arc<ImageInfo>>
}

impl Ord for ImageGallery {
    fn cmp(&self, other: &ImageGallery) -> Ordering {
        self.path.cmp(&other.path)
    }
}

impl PartialOrd for ImageGallery {
    fn partial_cmp(&self, other: &ImageGallery) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ImageGallery {
    fn eq(&self, other: &ImageGallery) -> bool {
        self.path.eq(&other.path)
    }
}

impl Eq for ImageGallery {
}

impl ImageGallery {
    pub fn get_name(&self) -> String {
        self.path.file_name()
            .and_then(|x| x.to_str())
            .map(|x| x.to_string())
            .unwrap_or("".to_string())
    }

    pub fn get_parent(&self) -> Option<String> {
        self.path.parent()
            .and_then(|x| x.to_str())
            .map(|x| x.to_string())
    }

    pub fn get_path(&self) -> String {
        self.path.to_str()
            .map(|x| x.to_string())
            .unwrap_or("".to_string())
    }

    pub fn find_gallery_from_name(&self, search_path: &PathBuf) -> Option<Arc<ImageGallery>> {
        for gallery in &self.sub_galleries {
            if gallery.path.as_path() == search_path.as_path() {
                return Some(gallery.clone());
            } else if search_path.starts_with(&gallery.path) {
                match gallery.find_gallery_from_name(search_path) {
                    Some(x) => return Some(x),
                    None => continue
                }
            }
        }

        None
    }
}

pub struct ImageFile {
    pub path: PathBuf,
    pub image: DynamicImage,
    pub hash: String
}

impl ImageFile {
    pub fn build_from_path(file: &PathBuf) -> IoResult<ImageFile> {
        let img = open_image(file)
            .or(build_io_error("Failed to open image"))?;
        let hash = hash_file(file)?;

        Ok(ImageFile {
            path: file.clone(),
            image: img,
            hash: hash
        })
    }

    pub fn scale_and_save(&self, max_width: u32, max_height: u32, dir: &PathBuf) -> IoResult<()> {
        let mut name = dir.clone();
        name.push(self.hash.clone() + ".jpg");

        if name.exists() {
            return Ok(());
        }

        let preview = self.image.resize(max_width, max_height, image::FilterType::CatmullRom);
        File::create(&name).and_then(|mut file| {
            preview.save(&mut file, image::ImageFormat::JPEG)
                 .or(Err(IoError::new(ErrorKind::Other, "Failed to write image data")))
        })
    }

    pub fn build_info(&self) -> Option<ImageInfo> {
        let file_name = match self.path.to_str() {
            Some(x) => x,
            None => return None
        };

        let (width, height) = self.image.dimensions();
        Some(ImageInfo {
            id: 0,
            name: file_name.to_string(),
            hash: self.hash.clone(),
            width: width,
            height: height,
            img_type: "JPEG".to_string()
        })
    }
}
