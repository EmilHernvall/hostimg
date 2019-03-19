use std::cmp::{Ordering, PartialOrd};
use std::error::Error;
use std::path::PathBuf;
use std::rc::Rc;
use std::result::Result;
use std::sync::mpsc;
use std::thread;
use std::fmt;

use serde_derive::Serialize;

use rusqlite::{Connection, Row, types::ToSql};

#[derive(Clone, Serialize)]
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

#[derive(Debug)]
pub enum DataStoreError {
    Connection(rusqlite::Error),
    Setup(rusqlite::Error),
    Execute(String, rusqlite::Error),
    QueryMap(rusqlite::Error),
    RowMap(rusqlite::Error),
    ChannelSend,
    ChannelReceive(Box<dyn Error + Send + Sync + 'static>),
}

impl Error for DataStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl fmt::Display for DataStoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub trait IntoModel {
    fn into(row: &Row) -> Self;
}

impl IntoModel for ImageInfo {
    fn into(row: &Row) -> ImageInfo {
        ImageInfo {
            id: row.get(0),
            name: row.get(1),
            hash: row.get(2),
            width: row.get(3),
            height: row.get(4),
            img_type: row.get(5),
        }
    }
}

pub trait Queryable {
    fn query_one<T>(&self, sql: &str, params: &[&ToSql]) -> Result<Option<T>, DataStoreError>
        where T: IntoModel;
    fn query_many<T>(&self, sql: &str, params: &[&ToSql]) -> Result<Vec<T>, DataStoreError>
        where T: IntoModel;
}

impl Queryable for Connection {
    fn query_many<T>(&self, sql: &str, params: &[&ToSql]) -> Result<Vec<T>, DataStoreError>
        where T: IntoModel
    {
        self
            .prepare(sql)
            .map_err(|e| DataStoreError::Execute(sql.to_string(), e))
            .and_then(|mut stmt| {
                stmt
                    .query_map(params, |row| IntoModel::into(row))
                    .map_err(|e| DataStoreError::QueryMap(e))?
                    .map(|item| item.map_err(|e| DataStoreError::RowMap(e)))
                    .collect::<Result<Vec<_>, DataStoreError>>()
            })
    }

    fn query_one<T>(&self, sql: &str, params: &[&ToSql]) -> Result<Option<T>, DataStoreError>
        where T: IntoModel
    {
        self.query_many(sql, params)
            .map(|res| res.into_iter().next())
    }
}

#[derive(Clone)]
pub struct DataStore {
    channel: mpsc::Sender<DbClosure>,
}

impl DataStore {
    pub fn new(data_dir: &PathBuf) -> Result<DataStore, DataStoreError> {
        let mut db_file = data_dir.clone();
        db_file.push("hostimg.db");

        let setup_db = !db_file.exists();

        let conn = Connection::open(db_file).map_err(|e| DataStoreError::Connection(e))?;

        if setup_db {
            conn.execute_batch(include_str!("schema.sql"))
                .map_err(|e| DataStoreError::Setup(e))?;
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

    pub fn with_conn<F, T>(&self, callback: F) -> Result<T, DataStoreError>
        where F: Fn(Rc<Connection>) -> Result<T, DataStoreError> + Send + 'static,
              T: Send + 'static
    {
        let (sender, receiver) = mpsc::channel::<Result<T, DataStoreError>>();

        self.channel
            .send(Box::new(move |conn: Rc<Connection>| {
                let res = callback(conn);

                if let Err(e) = sender.send(res) {
                    eprintln!("Failed to send datastore result: {:?}", e);
                }
            }))
            .map_err(|_e| DataStoreError::ChannelSend)?;

        receiver
            .recv()
            .map_err(|e| DataStoreError::ChannelReceive(Box::new(e)))?
    }

    pub fn find_image_by_id(&self, id: u32) -> Result<Option<ImageInfo>, DataStoreError> {
        self.with_conn(move |conn: Rc<Connection>| {
            conn.query_one("SELECT * FROM image WHERE image_id = ?1", &[&id])
        })
    }

    pub fn find_image_by_name(&self, name: String) -> Result<Option<ImageInfo>, DataStoreError> {
        self.with_conn(move |conn: Rc<Connection>| {
            conn.query_one("SELECT * FROM image WHERE image_name = ?1", &[&name])
        })
    }

    pub fn save_image(&self, info: ImageInfo) -> Result<i32, DataStoreError> {
        self.with_conn(move |conn: Rc<Connection>| {
            let sql = "INSERT INTO image (image_name, image_hash, image_width, image_height, image_type) VALUES (?1, ?2, ?3, ?4, ?5)";

            conn.execute(
                sql,
                &[
                    &info.name,
                    &info.hash,
                    &info.width,
                    &info.height,
                    &info.img_type,
                ],
            )
            .map_err(|e| DataStoreError::Execute(sql.to_string(), e))
        })
    }
}
