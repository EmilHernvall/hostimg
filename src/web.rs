use std::error::Error;
use std::sync::Arc;
use std::thread;

use regex::Regex;
use tiny_http::{Response, Server, StatusCode};
use lazy_static::lazy_static;

use crate::context::ServerContext;
use crate::gallery;

fn hex_to_num(c: char) -> u8 {
    match c {
        '0'...'9' => (c as u8) - (b'0' as u8),
        'a'...'f' => (c as u8) - (b'a' as u8) + 0xA,
        'A'...'F' => (c as u8) - (b'A' as u8) + 0xA,
        _ => 0,
    }
}

pub fn url_decode(instr: &str) -> String {
    let src_buffer = instr.as_bytes();

    let mut pos = 0;
    let len = instr.len();
    let mut buffer = String::new();
    while pos < len {
        let cur = src_buffer[pos] as char;
        if cur == '%' {
            let a = hex_to_num(src_buffer[pos + 1] as char);
            let b = hex_to_num(src_buffer[pos + 2] as char);
            let new_char = ((a << 4) | b) as char;
            buffer.push(new_char);
            pos += 2;
        } else {
            buffer.push(cur);
        }

        pos += 1;
    }

    buffer
}

#[derive(Debug)]
pub enum WebError {
    MissingParam,
    InvalidParam,
    NotFound,
    Other(Box<dyn Error + Send + Sync + 'static>),
}

pub fn run_server(context: ServerContext) {

    lazy_static!{
        static ref GALLERY_PATTERN : Regex = Regex::new(r"^/gallery$|^/gallery/(.*)$").unwrap();
        static ref IMAGE_PATTERN : Regex = Regex::new(r"^/image/(.+)/(.+)$").unwrap();
        static ref IMAGE_META_PATTERN : Regex = Regex::new(r"^/image/meta/([0-9]+)$").unwrap();
    }

    let webserver = match Server::http(("0.0.0.0", context.port)) {
        Ok(x) => x,
        Err(e) => {
            println!("Failed to start web server: {:?}", e);
            return;
        }
    };

    let webserver = Arc::new(webserver);

    for _ in 0..context.server_threads {
        let webserver = webserver.clone();
        let context = context.clone();

        thread::spawn(move || loop {
            let request = match webserver.recv() {
                Ok(x) => x,
                Err(e) => {
                    println!("Failed to retrieve request: {:?}", e);
                    continue;
                }
            };

            let context = context.clone();

            let url = request.url().to_string();
            println!("HTTP {:?} {:?}", request.method(), url);

            let result = if let Some(caps) = GALLERY_PATTERN.captures(&url) {
                let gallery = caps
                    .get(1)
                    .map(|x| url_decode(x.as_str()))
                    .map(|x| x.into());

                gallery::gallery_action(context, gallery)
            } else if let Some(caps) = IMAGE_PATTERN.captures(&url) {
                let hash = caps.get(1).map(|x| x.as_str().to_string());

                let img_size = caps.get(2).map(|x| x.as_str()).unwrap_or("thumb");

                gallery::image_action(context, hash, img_size)
            } else if let Some(caps) = IMAGE_META_PATTERN.captures(&url) {
                let id = caps.get(1).map(|x| x.as_str());

                gallery::image_meta_action(context, id)
            } else {
                Ok(Response::empty(StatusCode(404)).boxed())
            };

            let response_result = match result {
                Ok(response) => request.respond(response),
                Err(WebError::MissingParam) => request.respond(Response::empty(StatusCode(400))),
                Err(WebError::InvalidParam) => request.respond(Response::empty(StatusCode(400))),
                Err(WebError::NotFound) => request.respond(Response::empty(StatusCode(404))),
                Err(WebError::Other(e)) => {
                    eprintln!("Unexpected error servicing request: {:?}", e);
                    request.respond(Response::empty(StatusCode(500)))
                },
            };

            if let Err(e) = response_result {
                eprintln!("Response failed: {:?}", e);
            }
        });
    }
}
