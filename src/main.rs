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
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;


use multipart::server::{Multipart};
use tera::{Context, Tera};
use tiny_http::{Server, Method, Header, StatusCode, Response};
use log::{debug, error, info, warn};
use url::form_urlencoded;
use crate::config::setup::DeviceConfiguration;
use crate::logging::logging_util::setup_logging;
use crate::rfid::rfid_manger::{is_raspberry_pi, Rfid};

use crate::video_handler::media_manager::VlcManager;
use crate::web_server::file_action_handler::{route_action_form};



fn main() {
    let project_dir = current_dir().unwrap();

    let dev_config = DeviceConfiguration::load(project_dir.join("config/Config.yaml"));

    setup_logging(&dev_config).unwrap();
    info!("Starting Server!");

    let server = Server::http("0.0.0.0:8000").unwrap();

    let media_manager = VlcManager::new();

    let rfid = Rfid::new(media_manager.get_command_channel(), dev_config.clone());

    let mut tera = Tera::default();

    for mut request in server.incoming_requests() {
        debug!("received request! method: {:?}, url: {:?}, headers: {:?}",
             request.method(),
             request.url(),
             request.headers()
    );
        info!("Received request from {}: {:?}", request.remote_addr().unwrap(), request);

        match request.method() {
            Method::Get => {
                if request.url().contains("filename") {
                    if let Some((_, filename)) = request.url().split_once('='){
                        let decoded_filename = form_urlencoded::parse(filename.as_bytes())
                            .map(|(key, value)| value.into_owned())
                            .collect::<Vec<String>>()
                            .pop()
                            .unwrap_or_else(|| String::new());
                        info!("Decoded filename: {}", decoded_filename);

                        let filepath = project_dir.join("files").join(decoded_filename);

                        if let Ok(media) = File::open(filepath.clone()){
                            info!("Sending file to client");
                            request.respond(Response::from_file(media)).unwrap();
                            info!("Sent file to client");
                            continue;
                        } else {
                            error!("Failed to find file at path: {}", filepath.display())
                        }

                    }

                }
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
                    "/reboot" => {
                        if is_raspberry_pi() {
                            info!("Rebooting...");
                            request.respond(Response::from_string("")).unwrap();
                            Command::new("sudo")
                                .arg("reboot")
                                .arg("-f")
                                .output()
                                .expect("failed to execute reboot command");
                            panic!();
                        }else {
                            warn!("Did not reboot because it is not on a pi");
                        }
                    }
                    &_ => {}
                }
            }
            _ => {}
        }

        let contents = include_str!("../pages/index.html");

        tera.add_raw_template("index.html", &contents).expect("TODO: panic message");

        let paths = fs::read_dir(project_dir.join("files"))
            .expect("Should Have been a files DIR")
            .map(|entry| entry.unwrap().path().file_name().unwrap().to_str().unwrap().to_owned())
            .filter(|item| (!item.contains(".png") || item.eq("idle.png")))
            .collect::<Vec<_>>();

        let mut context = Context::new();
        context.insert("items", &paths);
        context.insert("deviceId", &dev_config.device_uuid);


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

}


