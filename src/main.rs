// This file is an example for vlc-rs, licensed under CC0.
// https://creativecommons.org/publicdomain/zero/1.0/deed

mod vlc_handler;

extern crate vlc;

use std::{fs, io};
use std::env::current_dir;

use std::fs::{File};
use std::io::{Read};


use multipart::server::{Multipart};
use tera::{Context, Tera};
use tiny_http::{Server, Method, Header, StatusCode, Response};
use log::{info};
use crate::vlc_handler::vlc_manager::Command::Play;
use crate::vlc_handler::vlc_manager::VlcManager;


fn main() {
    log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();


    info!("{}","-".repeat(50));
    info!("Starting Server!");


    let server = Server::http("0.0.0.0:8000").unwrap();

    let project_dir = current_dir().unwrap();

    let vlc_manager = VlcManager::new();

    let mut tera = Tera::default();


    for mut request in server.incoming_requests() {
        info!("received request! method: {:?}, url: {:?}, headers: {:?}",
                 request.method(),
                 request.url(),
                 request.headers()
        );
        match request.method() {
            Method::Get => {
                if request.url() == "/play" {
                    vlc_manager.send_command(Play).unwrap();
                    //command_tx.send(PLAY).unwrap();
                }else if request.url() == "/pause" {
                    //command_tx.send(PAUSE).unwrap();
                }else {
                    //let path = PathBuf::from(filedir.as_path().join("Flight Footage.mp4"));
                    //mdp.set_media(&md);
                    //mdp.play().unwrap();

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

}


