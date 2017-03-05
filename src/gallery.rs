use std::io::Result;
use std::sync::Arc;
use std::collections::BTreeMap;
use std::fs::File;
use std::time::UNIX_EPOCH;

use regex::{Regex,Captures};
use tiny_http::{Response, Header, HeaderField, Request};
use rustc_serialize::json::{ToJson, Json};
use chrono::prelude::*;
use chrono::Duration;
use handlebars::Handlebars;

use context::ServerContext;
use web::{WebServer,Action,url_decode,error_response};

pub struct GalleryAction {
    context: Arc<ServerContext>
}

impl GalleryAction {
    pub fn new(context: Arc<ServerContext>) -> GalleryAction {
        GalleryAction {
            context: context
        }
    }
}

impl Action for GalleryAction {
    fn get_regex(&self) -> Regex {
        Regex::new(r"^/gallery$|^/gallery/(.*)$").unwrap()
    }

    fn initialize(&self, server: &mut WebServer) -> Result<()> {
        let tpl_data = include_str!("templates/gallery.html").to_string();
        server.register_template("gallery", tpl_data)?;

        Ok(())
    }

    fn handle(&self,
              request: Request,
              caps: &Captures,
              handlebars: Arc<Handlebars>) -> Result<()> {

        let root_gallery = match self.context.get_root_gallery() {
            Ok(ref x) => x.clone(),
            Err(_) => return error_response(request, "No root gallery found")
        };

        let gallery = caps.get(1)
            .map(|x| url_decode(x.as_str()))
            .map(|x| x.into())
            .and_then(|x| root_gallery.find_gallery_from_name(&x))
            .unwrap_or(root_gallery);

        let mut sub_galleries = Vec::new();
        for sub_gallery in &gallery.sub_galleries {
            let mut gallery_dict = BTreeMap::new();
            gallery_dict.insert("path".to_string(), sub_gallery.get_path().to_json());
            gallery_dict.insert("name".to_string(), sub_gallery.get_name().to_json());
            sub_galleries.push(Json::Object(gallery_dict));
        }

        let sub_galleries = Json::Array(sub_galleries);

        let mut images = Vec::new();
        for image in &gallery.images {
            let mut image_dict = BTreeMap::new();
            image_dict.insert("name".to_string(), image.name.to_json());
            image_dict.insert("hash".to_string(), image.hash.to_json());
            image_dict.insert("width".to_string(), image.width.to_json());
            image_dict.insert("height".to_string(), image.height.to_json());
            images.push(Json::Object(image_dict));
        }

        let images = Json::Array(images);

        let mut result_dict = BTreeMap::new();
        result_dict.insert("name".to_string(), gallery.get_name().to_json());
        if let Some(parent) = gallery.get_parent() {
            result_dict.insert("has_parent".to_string(), true.to_json());
            result_dict.insert("parent".to_string(), parent.to_json());
        }
        result_dict.insert("sub_galleries".to_string(), sub_galleries);
        result_dict.insert("images".to_string(), images);
        let result_obj = Json::Object(result_dict);

        let html_data = match handlebars.render("gallery", &result_obj).ok() {
            Some(x) => x,
            None => return error_response(request, "Failed to encode response")
        };

        let mut response = Response::from_string(html_data);
        response.add_header(Header{
            field: "Content-Type".parse::<HeaderField>().unwrap(),
            value: "text/html".parse().unwrap()
        });
        return request.respond(response);
    }
}

pub struct ImageAction {
    context: Arc<ServerContext>
}

impl ImageAction {
    pub fn new(context: Arc<ServerContext>) -> ImageAction {
        ImageAction {
            context: context
        }
    }
}

impl Action for ImageAction {
    fn get_regex(&self) -> Regex {
        Regex::new(r"^/image/(.+)/(.+)$").unwrap()
    }

    fn initialize(&self, _: &mut WebServer) -> Result<()> {
        Ok(())
    }

    fn handle(&self,
              request: Request,
              caps: &Captures,
              _: Arc<Handlebars>) -> Result<()> {

        let hash = match caps.get(1).map(|x| x.as_str()).map(|x| x.to_string()) {
            Some(x) => x,
            None => return error_response(request, "No hash specified")
        };

        let img_size = caps.get(2)
            .map(|x| x.as_str())
            .unwrap_or("thumb");

        let mut path = match img_size {
            "thumb" => self.context.thumb_dir.clone(),
            "preview" => self.context.preview_dir.clone(),
            _ => return error_response(request, "Unknown image size requested")
        };
        path.push(hash + ".jpg");

        let file = File::open(&path)?;

        let mut response = Response::from_file(file);

        response.add_header(Header{
            field: "Cache-Control".parse::<HeaderField>().unwrap(),
            value: "private, max-age=31536000".parse().unwrap()
        });

        response.add_header(Header{
            field: "Content-Type".parse::<HeaderField>().unwrap(),
            value: "image/jpeg".parse().unwrap()
        });

        if let Ok(fixed) = path.metadata().and_then(|x| x.modified()) {
            if let Ok(time) = fixed.duration_since(UNIX_EPOCH) {
                let modified = NaiveDateTime::from_timestamp(time.as_secs() as i64, 0);
                let modified_formatted = modified.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
                response.add_header(Header{
                    field: "Last-Modified".parse::<HeaderField>().unwrap(),
                    value: modified_formatted.parse().unwrap()
                });
            }
        }

        let expires = UTC::now() + Duration::days(365);
        let expires_formatted = expires.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        response.add_header(Header{
            field: "Expires".parse::<HeaderField>().unwrap(),
            value: expires_formatted.parse().unwrap()
        });

        return request.respond(response);
    }
}
