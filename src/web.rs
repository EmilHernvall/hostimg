use std::io::{Error, ErrorKind, Result};
use std::sync::Arc;
use std::thread;

use handlebars::Handlebars;
use regex::{Captures, Regex};
use tiny_http::{Request, Response, Server, StatusCode};

use crate::context::ServerContext;

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

pub trait Action {
    fn get_regex(&self) -> Regex;
    fn initialize(&self, server: &mut WebServer) -> Result<()>;
    fn handle(
        &self,
        request: Request,
        path_match: &Captures,
        context: ServerContext,
        handlebars: Arc<Handlebars>,
    ) -> Result<()>;
}

struct Template {
    name: String,
    content: String,
}

pub type ThreadsafeAction = Box<Action + Send + Sync>;

pub struct WebServer {
    pub context: ServerContext,
    actions: Vec<Arc<ThreadsafeAction>>,
    templates: Vec<Template>,
}

impl WebServer {
    pub fn new(context: ServerContext) -> Result<WebServer> {
        let mut server = WebServer {
            context,
            templates: Vec::new(),
            actions: Vec::new(),
        };

        let tpl_data = include_str!("templates/layout.html").to_string();
        server.register_template("layout", tpl_data);

        Ok(server)
    }

    pub fn register_template(&mut self, name: &str, tpl_data: String) {
        self.templates.push(Template {
            name: name.to_string(),
            content: tpl_data,
        });
    }

    pub fn register_action(&mut self, action: ThreadsafeAction) -> Result<()> {
        action.initialize(self)?;
        self.actions.push(Arc::new(action));

        Ok(())
    }

    pub fn run_webserver(self, join: bool) {
        let mut handlebars = Handlebars::new();
        for template in self.templates {
            if let Err(e) =
                handlebars.register_template_string(template.name.as_str(), template.content)
            {
                println!("Failed to register template {}: {:?}", template.name, e);
            }
        }

        let handlebars = Arc::new(handlebars);

        let webserver = match Server::http(("0.0.0.0", self.context.port)) {
            Ok(x) => x,
            Err(e) => {
                println!("Failed to start web server: {:?}", e);
                return;
            }
        };

        let webserver = Arc::new(webserver);

        let mut guards = Vec::with_capacity(self.context.server_threads);
        for _ in 0..self.context.server_threads {
            let webserver = webserver.clone();
            let handlebars = handlebars.clone();
            let context = self.context.clone();

            let actions: Vec<_> = self.actions.iter().cloned().collect();

            let guard = thread::spawn(move || loop {
                let request = match webserver.recv() {
                    Ok(x) => x,
                    Err(e) => {
                        println!("Failed to retrieve request: {:?}", e);
                        continue;
                    }
                };

                println!("HTTP {:?} {:?}", request.method(), request.url());

                let context = context.clone();
                let matching_actions: Vec<Arc<ThreadsafeAction>> = actions
                    .iter()
                    .filter(|x| x.get_regex().is_match(&request.url()))
                    .cloned()
                    .collect();

                if matching_actions.is_empty() {
                    let response = Response::empty(StatusCode(404));
                    let _ = request.respond(response);
                } else {
                    let action = &matching_actions[0];
                    if let Some(caps) = action.get_regex().captures(&request.url().to_string()) {
                        let _ = action.handle(request, &caps, context, handlebars.clone());
                    }
                }
            });

            guards.push(guard);
        }

        if join {
            for guard in guards {
                let _ = guard.join();
            }
        }
    }
}

pub fn error_response(request: Request, error: &str) -> Result<()> {
    let response = Response::empty(StatusCode(400));
    let _ = request.respond(response);
    Err(Error::new(ErrorKind::InvalidInput, error))
}
