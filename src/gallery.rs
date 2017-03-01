use std::path::Path;
use std::io::{Result, Error, ErrorKind, Read};
use std::sync::Arc;
use std::collections::BTreeMap;
use std::fs::File;

use regex::{Regex,Captures};
use tiny_http::{Response, Header, HeaderField, Request, Method, StatusCode};
use rustc_serialize::json::{ToJson, Json};

use context::ServerContext;
use web::{WebServer,Action};

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

    fn initialize(&self, server: &mut WebServer) {
        let tpl_data = include_str!("templates/gallery.html").to_string();
        if !server.handlebars.register_template_string("gallery", tpl_data).is_ok() {
            println!("Failed to register gallery template");
            return;
        }
    }

    fn handle(&self,
              server: &WebServer,
              request: Request,
              caps: &Captures) -> Result<()> {

        let root_gallery = match self.context.root_gallery {
            Some(ref x) => x.clone(),
            None => return server.error_response(request, "No root gallery found")
        };

        let gallery = caps.get(1)
            .map(|x| Path::new(x.as_str()))
            .and_then(|x| root_gallery.find_gallery_from_name(x))
            .unwrap_or(root_gallery);

        println!("parent: {:?}", gallery.get_parent());

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

        let html_data = match server.handlebars.render("gallery", &result_obj).ok() {
            Some(x) => x,
            None => return server.error_response(request, "Failed to encode response")
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

    fn initialize(&self, server: &mut WebServer) {
    }

    fn handle(&self,
              server: &WebServer,
              request: Request,
              caps: &Captures) -> Result<()> {

        let hash = match caps.get(1).map(|x| x.as_str()).map(|x| x.to_string()) {
            Some(x) => x,
            None => return server.error_response(request, "No hash specified")
        };

        let img_size = caps.get(2)
            .map(|x| x.as_str())
            .unwrap_or("thumb");

        let mut path = match img_size {
            "thumb" => self.context.thumb_dir.clone(),
            "preview" => self.context.preview_dir.clone(),
            _ => return server.error_response(request, "Unknown image size requested")
        };
        path.push(hash + ".jpg");

        let file = File::open(path)?;

        let mut response = Response::from_file(file);
        response.add_header(Header{
            field: "Content-Type".parse::<HeaderField>().unwrap(),
            value: "image/jpeg".parse().unwrap()
        });
        return request.respond(response);
    }
}
