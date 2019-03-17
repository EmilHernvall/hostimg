extern crate image;
extern crate sha2;

use std::cmp::{Ordering, PartialOrd};
use std::collections::BTreeSet;
use std::fs::{read_dir, File};
use std::io::{self, BufReader, ErrorKind, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use image::{DynamicImage, GenericImage, ImageResult};
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use sha2::Digest;

use crate::context::{ContextError, ServerContext};
use crate::db::{DataStoreError, ImageInfo};

pub fn open_image(file: &Path) -> ImageResult<DynamicImage> {
    let file_obj = File::open(&file)?;
    let reader = BufReader::new(file_obj);
    image::load(reader, image::ImageFormat::JPEG)
}

pub fn hash_file(file: &Path) -> io::Result<String> {
    let mut file_obj = File::open(file)?;

    let mut buffer = [0; 4096];
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

#[derive(Debug)]
pub enum ScannerError {
    Charset,
    Io(io::Error),
    Context(ContextError),
    DataStore(DataStoreError),
    Fs,
}

impl From<io::Error> for ScannerError {
    fn from(other: io::Error) -> Self {
        ScannerError::Io(other)
    }
}

impl From<ContextError> for ScannerError {
    fn from(other: ContextError) -> Self {
        ScannerError::Context(other)
    }
}

impl From<DataStoreError> for ScannerError {
    fn from(other: DataStoreError) -> Self {
        ScannerError::DataStore(other)
    }
}

pub struct GalleryScanner {
    context: ServerContext,
    indexing_queue: Sender<PathBuf>,
    indexing_receiver: Option<Receiver<PathBuf>>,
}

fn process_image(context: &ServerContext, file: &Path) -> Result<Arc<ImageInfo>, ScannerError> {
    let file_name = file.to_str().ok_or(ScannerError::Charset)?;
    println!("Adding {}", file_name);

    let image_file = ImageFile::build_from_path(file.to_path_buf())?;

    image_file.scale_and_save(2048, 2048, &context.preview_dir)?;
    image_file.scale_and_save(256, 256, &context.thumb_dir)?;

    let info = image_file.build_info()?;

    context.datastore.save_image(info.clone())?;

    let parent = file
        .parent()
        .ok_or(ScannerError::Fs)?
        .strip_prefix(&context.gallery_dir)
        .map_err(|_| ScannerError::Fs)?
        .to_path_buf();

    let info = Arc::new(info);
    let op = GalleryModification::Add(info.clone());

    let root_gallery = context.get_root_gallery()?;
    let new_root = root_gallery.modify(&parent, op)?;

    context.set_root_gallery(new_root)?;

    Ok(info)
}

impl GalleryScanner {
    pub fn new(context: ServerContext) -> GalleryScanner {
        let (indexing_queue, indexing_receiver) = channel();

        GalleryScanner {
            context,
            indexing_queue,
            indexing_receiver: Some(indexing_receiver),
        }
    }

    pub fn process_images(&mut self) {
        let context = self.context.clone();

        let mut indexing_receiver = None;
        std::mem::swap(&mut self.indexing_receiver, &mut indexing_receiver);

        thread::spawn(move || {
            let indexing_receiver =
                indexing_receiver.expect("Failed to start indexing thread: Incoming queue missing");
            for file in indexing_receiver {
                match process_image(&context, &file) {
                    Ok(info) => println!("Completed processing: {:?}", info.name),
                    Err(e) => eprintln!("Failed to process {:?}: {:?}", file, e),
                }
            }
        });
    }

    pub fn scan(&mut self) -> Result<(), io::Error> {
        let gallery_dir = &self.context.gallery_dir.clone();

        let gallery = Arc::new(self.scan_recursive(gallery_dir, &is_image)?);

        self.context
            .set_root_gallery(gallery)
            .or(build_io_result("Failed to set root gallery"))?;

        Ok(())
    }

    fn scan_recursive<F>(&mut self, dir: &PathBuf, accept: &F) -> Result<ImageGallery, io::Error>
    where
        F: Fn(&Path) -> bool,
    {
        let local_path = dir
            .strip_prefix(&self.context.gallery_dir)
            .or(build_io_result("Failed to strip directory prefix"))?
            .to_path_buf();

        let mut new_gallery = ImageGallery::new(local_path);

        for entry in read_dir(dir)?.filter_map(|x| x.ok()) {
            let filetype = entry.file_type()?;
            let p = entry.path();
            if filetype.is_dir() {
                match self.scan_recursive(&p, accept) {
                    Ok(gallery) => {
                        if gallery.imagecount > 0 {
                            new_gallery.imagecount += gallery.imagecount;
                            new_gallery.sub_galleries.insert(Arc::new(gallery));
                        }
                    }
                    Err(e) => println!("Failed to add gallery {:?}", e),
                }
            } else if filetype.is_file() && accept(&p) {
                match self.find_file(&p) {
                    Ok(Some(info)) => {
                        new_gallery.imagecount += 1;
                        new_gallery.images.insert(Arc::new(info));
                    }
                    Ok(None) => {
                        println!("Deferring indexing of {:?}", p);
                        self.indexing_queue
                            .send(p.clone())
                            .or(build_io_result("Failed to send file to indexing thread"))?;
                    }
                    Err(e) => println!("Failed to add image {:?}", e),
                }
            }
        }

        println!(
            "Finished scanning directory {:?} with {} images",
            dir, new_gallery.imagecount
        );

        Ok(new_gallery)
    }

    fn find_file(&mut self, file: &PathBuf) -> Result<Option<ImageInfo>, ScannerError> {
        let file_name = file.to_str().ok_or(ScannerError::Charset)?;

        let query_result = self
            .context
            .datastore
            .find_image_by_name(file_name.to_string())?;

        Ok(query_result)
    }

    fn handle_update(&mut self, event: DebouncedEvent) -> Result<(), io::Error> {
        let root_gallery = self
            .context
            .get_root_gallery()
            .or(build_io_result("Failed to get root gallery"))?;

        match event {
            DebouncedEvent::Create(ref path) => {
                if !is_image(path) {
                    return Ok(());
                }

                if let Some(info) = self
                    .find_file(path)
                    .or(build_io_result("Failed to add file"))?
                {
                    let parent = match path
                        .parent()
                        .and_then(|x| x.strip_prefix(&self.context.gallery_dir).ok())
                    {
                        Some(x) => x.to_path_buf(),
                        None => return build_io_result("Path has no parent"),
                    };

                    println!("Found new image: {:?}", path);
                    let op = GalleryModification::Add(Arc::new(info));

                    let new_root = root_gallery.modify(&parent, op)?;
                    self.context
                        .set_root_gallery(new_root)
                        .or(build_io_result("Failed to set root gallery"))?;
                } else {
                    self.indexing_queue
                        .send(path.clone())
                        .or(build_io_result("Failed to send file to indexing thread"))?;
                }
            }
            DebouncedEvent::Remove(ref path) => {
                if !is_image(path) {
                    return Ok(());
                }

                let info = Arc::new(
                    self.find_file(path)
                        .or(build_io_result("Failed to add file"))?
                        .ok_or(build_io_error("File not found"))?,
                );

                let parent = match path
                    .parent()
                    .and_then(|x| x.strip_prefix(&self.context.gallery_dir).ok())
                {
                    Some(x) => x.to_path_buf(),
                    None => return build_io_result("Path has no parent"),
                };

                println!("Detected removed image: {:?}", path);
                let op = GalleryModification::Remove(info);

                let new_root = root_gallery.modify(&parent, op)?;
                self.context
                    .set_root_gallery(new_root)
                    .or(build_io_result("Failed to set root gallery"))?;
            }
            DebouncedEvent::Rename(ref from_path, ref to_path) => {
                if !is_image(from_path) {
                    return Ok(());
                }

                println!("Detected rename: {:?} - {:?}", from_path, to_path);

                let from_info = Arc::new(
                    self.find_file(from_path)
                        .or(build_io_result("Failed to add file"))?
                        .ok_or(build_io_error("File not found"))?,
                );
                let from_parent = match from_path
                    .parent()
                    .and_then(|x| x.strip_prefix(&self.context.gallery_dir).ok())
                {
                    Some(x) => x.to_path_buf(),
                    None => return build_io_result("Path has no parent"),
                };

                self.indexing_queue
                    .send(to_path.clone())
                    .or(build_io_result("Failed to send file to indexing thread"))?;

                let new_root =
                    root_gallery.modify(&from_parent, GalleryModification::Remove(from_info))?;
                self.context
                    .set_root_gallery(new_root)
                    .or(build_io_result("Failed to set root gallery"))?;
            }
            DebouncedEvent::Write(_) => {}
            DebouncedEvent::Rescan => {}
            _ => {}
        }

        Ok(())
    }

    pub fn monitor(mut self) -> Result<(), io::Error> {
        let (tx, rx) = channel();

        let mut watcher =
            watcher(tx, Duration::from_secs(10)).or(build_io_result("Failed to create watcher"))?;
        watcher
            .watch(&self.context.gallery_dir, RecursiveMode::Recursive)
            .or(build_io_result("Failed to register watch"))?;

        loop {
            match rx
                .recv()
                .or(build_io_result("Failed to read event"))
                .and_then(|event| self.handle_update(event))
            {
                Ok(_) => {}
                Err(e) => println!("Watch error: {:?}", e),
            }
        }
    }
}

fn build_io_error(message: &str) -> io::Error {
    io::Error::new(ErrorKind::Other, message)
}

fn build_io_result<T>(message: &str) -> Result<T, io::Error> {
    Err(io::Error::new(ErrorKind::Other, message))
}

pub struct ImageGallery {
    pub path: PathBuf,
    pub sub_galleries: BTreeSet<Arc<ImageGallery>>,
    pub images: BTreeSet<Arc<ImageInfo>>,
    pub imagecount: u32,
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

impl Eq for ImageGallery {}

#[derive(Clone)]
pub enum GalleryModification {
    Add(Arc<ImageInfo>),
    Remove(Arc<ImageInfo>),
}

impl ImageGallery {
    pub fn new(path: PathBuf) -> ImageGallery {
        ImageGallery {
            path: path,
            sub_galleries: BTreeSet::new(),
            images: BTreeSet::new(),
            imagecount: 0,
        }
    }

    pub fn get_name(&self) -> String {
        self.path
            .file_name()
            .and_then(|x| x.to_str())
            .map(|x| x.to_string())
            .unwrap_or("".to_string())
    }

    pub fn get_parent(&self) -> Option<String> {
        self.path
            .parent()
            .and_then(|x| x.to_str())
            .map(|x| x.to_string())
    }

    pub fn get_path(&self) -> String {
        self.path
            .to_str()
            .map(|x| x.to_string())
            .unwrap_or("".to_string())
    }

    fn build_next_path_step(&self, dir_path: &PathBuf) -> Option<PathBuf> {
        let next_component = match dir_path
            .strip_prefix(&self.path)
            .ok()
            .and_then(|x| x.components().next())
        {
            Some(x) => x,
            None => return None,
        };

        match next_component {
            Component::Normal(next_component) => {
                let mut new_subpath = self.path.clone();
                new_subpath.push(next_component);

                Some(new_subpath)
            }
            _ => None,
        }
    }

    pub fn modify(
        &self,
        dir_path: &PathBuf,
        op: GalleryModification,
    ) -> Result<Arc<ImageGallery>, io::Error> {
        let mut new_self = ImageGallery::new(self.path.clone());
        let mut found_gallery = false;
        for subgallery in &self.sub_galleries {
            let new_subgallery = if dir_path.starts_with(&subgallery.path) {
                found_gallery = true;
                subgallery.modify(dir_path, op.clone())?
            } else {
                subgallery.clone()
            };

            if new_subgallery.imagecount > 0 {
                new_self.imagecount += new_subgallery.imagecount;
                new_self.sub_galleries.insert(new_subgallery);
            }
        }

        for image in &self.images {
            new_self.imagecount += 1;
            new_self.images.insert(image.clone());
        }

        if dir_path.as_path() == self.path.as_path() {
            match op {
                GalleryModification::Add(info) => {
                    new_self.images.insert(info.clone());
                    new_self.imagecount += 1;
                }
                GalleryModification::Remove(info) => {
                    if new_self.images.remove(&info) {
                        new_self.imagecount -= 1;
                    }
                }
            }
        } else if !found_gallery {
            match self.build_next_path_step(dir_path) {
                Some(new_subpath) => {
                    let new_subgallery =
                        ImageGallery::new(new_subpath).modify(dir_path, op.clone())?;
                    new_self.imagecount += new_subgallery.imagecount;
                    new_self.sub_galleries.insert(new_subgallery);
                }
                None => {
                    return build_io_result(
                        "Failed to find next path step when rebuilding gallery structure",
                    );
                }
            }
        }

        Ok(Arc::new(new_self))
    }

    pub fn find_gallery_from_name(&self, search_path: &PathBuf) -> Option<Arc<ImageGallery>> {
        for gallery in &self.sub_galleries {
            if gallery.path.as_path() == search_path.as_path() {
                return Some(gallery.clone());
            } else if search_path.starts_with(&gallery.path) {
                match gallery.find_gallery_from_name(search_path) {
                    Some(x) => return Some(x),
                    None => continue,
                }
            }
        }

        None
    }
}

pub struct ImageFile {
    pub path: PathBuf,
    pub image: DynamicImage,
    pub hash: String,
}

impl ImageFile {
    pub fn build_from_path(path: PathBuf) -> Result<ImageFile, io::Error> {
        let img = open_image(&path).or(build_io_result("Failed to open image"))?;
        let hash = hash_file(&path)?;

        Ok(ImageFile {
            path,
            image: img,
            hash: hash,
        })
    }

    pub fn scale_and_save(
        &self,
        max_width: u32,
        max_height: u32,
        dir: &PathBuf,
    ) -> Result<(), io::Error> {
        let mut name = dir.clone();
        name.push(self.hash.clone() + ".jpg");

        if name.exists() {
            return Ok(());
        }

        let preview = self
            .image
            .resize(max_width, max_height, image::FilterType::CatmullRom);
        File::create(&name).and_then(|mut file| {
            preview
                .save(&mut file, image::ImageFormat::JPEG)
                .or(Err(io::Error::new(
                    ErrorKind::Other,
                    "Failed to write image data",
                )))
        })
    }

    pub fn build_info(&self) -> Result<ImageInfo, ScannerError> {
        let file_name = self.path.to_str().ok_or(ScannerError::Charset)?;

        let (width, height) = self.image.dimensions();
        Ok(ImageInfo {
            id: 0,
            name: file_name.to_string(),
            hash: self.hash.clone(),
            width: width,
            height: height,
            img_type: "JPEG".to_string(),
        })
    }
}
