use std::cmp::{Ordering, PartialOrd};
use std::io::Error as IoError;
use std::io::{ErrorKind, Result};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use rusqlite::Connection;

fn build_error(message: &str) -> IoError {
    IoError::new(ErrorKind::Other, message)
}

#[derive(Clone)]
pub struct ImageInfo {
    pub id: u32,
    pub name: String,
    pub hash: String,
    pub width: u32,
    pub height: u32,
    pub img_type: String,
}

impl Ord for ImageInfo {
    fn cmp(&self, other: &ImageInfo) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for ImageInfo {
    fn partial_cmp(&self, other: &ImageInfo) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ImageInfo {
    fn eq(&self, other: &ImageInfo) -> bool {
        self.name.eq(&other.name)
    }
}

impl Eq for ImageInfo {}

type DbClosure = Box<dyn Fn(Rc<Connection>) + Send + 'static>;

#[derive(Clone)]
pub struct DataStore {
    channel: mpsc::Sender<DbClosure>,
}

impl DataStore {
    pub fn new(data_dir: &PathBuf) -> Result<DataStore> {
        let mut db_file = data_dir.clone();
        db_file.push("hostimg.db");

        let setup_db = !db_file.exists();

        let conn = Connection::open(db_file).or(Err(build_error("Failed to open database")))?;

        if setup_db {
            conn.execute(
                "CREATE TABLE image (
                image_id INTEGER PRIMARY KEY,
                image_name TEXT NOT NULL,
                image_hash TEXT NOT NULL,
                image_width INTEGER NOT NULL,
                image_height INTEGER NOT NULL,
                image_type TEXT NOT NULL
            )",
                &[],
            )
            .or(Err(build_error("Failed to create table 'image'")))?;
        }

        let (channel, receiver) = mpsc::channel::<DbClosure>();

        // TODO: introduce some parallellism by dividng work across a threadpool
        thread::spawn(move || {
            let conn = Rc::new(conn);
            for closure in receiver {
                closure(conn.clone());
            }
        });

        Ok(DataStore { channel })
    }

    pub fn find_image_by_name(&self, name: String) -> Result<Option<ImageInfo>> {
        let (sender, receiver) = mpsc::channel::<Result<Vec<ImageInfo>>>();

        self.channel
            .send(Box::new(move |conn: Rc<Connection>| {
                let res = conn
                    .prepare("SELECT * FROM image WHERE image_name = ?1")
                    .map_err(|_e| build_error("Failed to prepare statement"))
                    .and_then(|mut stmt| {
                        let mapped_rows = stmt
                            .query_map(&[&name], |row| ImageInfo {
                                id: row.get(0),
                                name: row.get(1),
                                hash: row.get(2),
                                width: row.get(3),
                                height: row.get(4),
                                img_type: row.get(5),
                            })
                            .map_err(|_e| build_error("Failed to execute query"))?;

                        mapped_rows
                            .map(|item| item.map_err(|_e| build_error("Could not map object")))
                            .collect::<Result<Vec<ImageInfo>>>()
                    });

                sender.send(res).unwrap();
            }))
            .unwrap();

        let res = receiver.recv().unwrap()?;

        match res.into_iter().next() {
            Some(x) => Ok(Some(x)),
            None => Ok(None),
        }
    }

    pub fn save_image(&mut self, info: ImageInfo) -> Result<i32> {
        let (sender, receiver) = mpsc::channel();

        self.channel
            .send(Box::new(move |conn: Rc<Connection>| {
                let res = conn
                    .execute(
                        "INSERT INTO image (image_name, image_hash, image_width,
                 image_height, image_type) VALUES (?1, ?2, ?3, ?4, ?5)",
                        &[
                            &info.name,
                            &info.hash,
                            &info.width,
                            &info.height,
                            &info.img_type,
                        ],
                    )
                    .or(Err(build_error("Failed to save row")));

                sender.send(res).unwrap();
            }))
            .unwrap();

        receiver.recv().unwrap()
    }
}
