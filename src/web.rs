use std::io::{Result, Error, ErrorKind};
use std::sync::Arc;

use regex::{Regex,Captures};
use tiny_http::{Server, Response, StatusCode, Request};
use handlebars::Handlebars;

use context::ServerContext;

fn hex_to_num(c: char) -> u8 {
    match c {
        '0'...'9' => (c as u8) - (b'0' as u8),
        'a'...'f' => (c as u8) - (b'a' as u8) + 0xA,
        'A'...'F' => (c as u8) - (b'A' as u8) + 0xA,
        _ => 0
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
            let a = hex_to_num(src_buffer[pos+1] as char);
            let b = hex_to_num(src_buffer[pos+2] as char);
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

pub trait Action {
    fn get_regex(&self) -> Regex;
    fn initialize(&self, server: &mut WebServer);
    fn handle(&self,
              server: &WebServer,
              request: Request,
              path_match: &Captures) -> Result<()>;
}

pub struct WebServer {
    pub context: Arc<ServerContext>,
    pub handlebars: Handlebars,
    pub actions: Vec<Box<Action>>
}

impl WebServer {

    pub fn new(context: Arc<ServerContext>) -> WebServer {
        let mut server = WebServer {
            context: context,
            handlebars: Handlebars::new(),
            actions: Vec::new()
        };

        let tpl_data = include_str!("templates/layout.html").to_string();
        if !server.handlebars.register_template_string("layout", tpl_data).is_ok() {
            println!("Failed to register layout template");
        }

        server
    }

    pub fn register_action(&mut self, action: Box<Action>) {
        action.initialize(self);
        self.actions.push(action);
    }

    pub fn run_webserver(self)
    {
        let webserver = match Server::http(("0.0.0.0", self.context.port)) {
            Ok(x) => x,
            Err(e) => {
                println!("Failed to start web server: {:?}", e);
                return;
            }
        };

        let webserver = Arc::new(webserver);

        for request in webserver.incoming_requests() {
            println!("HTTP {:?} {:?}", request.method(), request.url());

            let matching_actions : Vec<&Box<Action>> =
                self.actions.iter().filter(|x| x.get_regex().is_match(&request.url())).collect();

            if matching_actions.is_empty() {
                let response = Response::empty(StatusCode(404));
                let _ = request.respond(response);
            } else {
                let action = &matching_actions[0];
                if let Some(caps) = action.get_regex().captures(&request.url().to_string()) {
                    let _ = action.handle(&self, request, &caps);
                }
            }
        }
    }

    pub fn error_response(&self, request: Request, error: &str) -> Result<()>
    {
        let response = Response::empty(StatusCode(400));
        let _ = request.respond(response);
        Err(Error::new(ErrorKind::InvalidInput, error))
    }
}

