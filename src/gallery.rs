use std::fs::File;
use std::result::Result;
use std::time::UNIX_EPOCH;
use std::path::PathBuf;

use chrono::prelude::*;
use chrono::Duration;
use tiny_http::{Header, HeaderField, Response, ResponseBox};
use serde_json::json;

use crate::context::ServerContext;
use crate::web::WebError;

pub fn gallery_action(
    context: ServerContext,
    gallery: Option<PathBuf>,
) -> Result<ResponseBox, WebError> {
    let root_gallery = match context.get_root_gallery() {
        Ok(ref x) => x.clone(),
        Err(e) => return Err(WebError::Other(Box::new(e))),
    };

    let gallery = gallery.and_then(|x| root_gallery.find_gallery_from_name(&x))
        .unwrap_or(root_gallery);

    let mut sub_galleries = Vec::new();
    for sub_gallery in &gallery.sub_galleries {
        sub_galleries.push(json!({
            "path": sub_gallery.get_path(),
            "name": sub_gallery.get_name(),
        }));
    }

    let mut images = Vec::new();
    for image in &gallery.images {
        images.push(json!({
            "name": image.name,
            "hash": image.hash,
            "width": image.width,
            "height": image.height,
        }));
    }

    let result_obj = json!({
        "name": gallery.get_name(),
        "sub_galleries": sub_galleries,
        "images": images,
        "parent": gallery.get_parent(),
    });

    let json_data = serde_json::to_string(&result_obj)
        .map_err(|e| WebError::Other(Box::new(e)))?;

    let mut response = Response::from_string(json_data);
    response.add_header(Header{
        field: "Access-Control-Allow-Origin".parse::<HeaderField>().unwrap(),
        value: "*".parse().unwrap()
    });
    response.add_header(Header{
        field: "Content-Type".parse::<HeaderField>().unwrap(),
        value: "application/json".parse().unwrap()
    });

    Ok(response.boxed())
}

pub fn image_action(
    context: ServerContext,
    hash: Option<String>,
    img_size: &str,
) -> Result<ResponseBox, WebError> {

    let hash = match hash {
        Some(x) => x.to_string(),
        None => return Err(WebError::MissingParam),
    };

    let mut path = match img_size {
        "thumb" => context.thumb_dir.clone(),
        "preview" => context.preview_dir.clone(),
        _ => return Err(WebError::InvalidParam),
    };
    path.push(hash + ".jpg");

    let file = File::open(&path)
        .map_err(|_e| WebError::NotFound)?;

    let mut response = Response::from_file(file);

    response.add_header(Header {
        field: "Cache-Control".parse::<HeaderField>().unwrap(),
        value: "private, max-age=31536000".parse().unwrap(),
    });

    response.add_header(Header {
        field: "Content-Type".parse::<HeaderField>().unwrap(),
        value: "image/jpeg".parse().unwrap(),
    });

    if let Ok(fixed) = path.metadata().and_then(|x| x.modified()) {
        if let Ok(time) = fixed.duration_since(UNIX_EPOCH) {
            let modified = NaiveDateTime::from_timestamp(time.as_secs() as i64, 0);
            let modified_formatted = modified.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
            response.add_header(Header {
                field: "Last-Modified".parse::<HeaderField>().unwrap(),
                value: modified_formatted.parse().unwrap(),
            });
        }
    }

    let expires = UTC::now() + Duration::days(365);
    let expires_formatted = expires.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    response.add_header(Header {
        field: "Expires".parse::<HeaderField>().unwrap(),
        value: expires_formatted.parse().unwrap(),
    });

    Ok(response.boxed())
}
