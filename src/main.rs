// This file is an example for vlc-rs, licensed under CC0.
// https://creativecommons.org/publicdomain/zero/1.0/deed

mod video_handler;
mod rfid;
mod web_server;
mod config;
mod logging;

use std::{fs, io, thread};
use std::env::current_dir;

use std::fs::{File};
use std::io::{Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};


use multipart::server::{Multipart};
use tera::{Context, Tera};
use tiny_http::{Server, Method, Header, StatusCode, Response};
use log::{debug, error, info};
use crate::config::setup::DeviceConfiguration;
use crate::logging::logging_util::setup_logging;
use crate::rfid::rfid_manger::Rfid;

use crate::video_handler::media_manager::VlcManager;
use crate::web_server::file_action_handler::{route_action_form};



fn main() {

    let term_received = Arc::new(AtomicBool::new(false));

    // Set up a signal handler for SIGTERM
    let term_received_clone = term_received.clone();
    ctrlc::set_handler(move || {
        term_received_clone.store(true, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");


    let project_dir = current_dir().unwrap();

    let dev_config = DeviceConfiguration::load(project_dir.join("config/Config.yaml"));

    setup_logging(&dev_config).unwrap();
    info!("Starting Server!");

    let server = Server::http("0.0.0.0:8000").unwrap();

    let media_manager = VlcManager::new();

    let rfid = Rfid::new(media_manager.get_command_channel(), dev_config.clone());

    let mut tera = Tera::default();

    let _server = thread::spawn(move || {
        for mut request in server.incoming_requests() {
            debug!("received request! method: {:?}, url: {:?}, headers: {:?}",
                 request.method(),
                 request.url(),
                 request.headers()
        );
            info!("Received request from {}: {:?}", request.remote_addr().unwrap(), request);

            match request.method() {
                Method::Get => {

                },
                Method::Post => {
                    match request.url() {
                        "/upload" => {
                            let boundary = request
                                .headers()
                                .iter()
                                .find(|h| h.field.as_str() == "Content-Type")
                                .map(|h| {
                                    let content_type = h.value.as_str();
                                    content_type.split("boundary=").last().unwrap().to_string()
                                });

                            let boundary = match boundary {
                                Some(boundary) => boundary,
                                None => {"".to_owned()},
                            };
                            let mut multipart = Multipart::with_body(request.as_reader(), &boundary);

                            while let Ok(Some(mut field)) = multipart.read_entry() {
                                let file_name = field
                                    .headers
                                    .filename
                                    .clone()
                                    .ok_or(io::Error::new(io::ErrorKind::InvalidInput, "No filename found"))
                                    .unwrap();

                                let file_path = project_dir.clone().join("files").join(file_name);

                                info!("Pulling file from client saving here: {}", file_path.as_path().to_str().unwrap());

                                let mut file = File::create(&file_path).unwrap();
                                io::copy(&mut field.data, &mut file).unwrap();
                            }
                        },
                        "/action" => {
                            match route_action_form(request, &media_manager, &rfid) {
                                Ok(action) => {
                                    info!("Media Action Form routed successfully. Action preformed : {:?}", action)
                                }
                                Err(error) => {
                                    error!("Routing of action form failed because: {:?}", error)
                                }
                            }
                            continue
                        }
                        &_ => {}
                    }
                }
                _ => {}
            }

            let mut file = File::open(project_dir.as_path().join("pages").join("index.html")).unwrap();
            let mut contents = String::new();
            file.read_to_string(&mut contents).unwrap();

            tera.add_raw_template("index.html", &contents).expect("TODO: panic message");

            let paths = fs::read_dir(project_dir.join("files"))
                .expect("Should Have been a files DIR")
                .map(|entry| entry.unwrap().path().file_name().unwrap().to_str().unwrap().to_owned())
                .filter(|item| (!item.contains(".png") || item.eq("idle.png")))
                .collect::<Vec<_>>();

            let mut context = Context::new();
            context.insert("items", &paths);

            let rendered = tera.render("index.html", &context).unwrap();


            let response = Response::new(
                StatusCode(200),
                vec![Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap()],
                rendered.as_bytes(),
                None,
                None
            );
            request.respond(response).expect("TODO: panic message");
        }
    });

    // Start the main loop
    loop {
        // Check if SIGTERM was received
        if term_received.load(Ordering::SeqCst) {
            println!("SIGTERM received, exiting main loop");
            break;
        }
        // Do some work here
    }

}


