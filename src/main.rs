// This file is an example for vlc-rs, licensed under CC0.
// https://creativecommons.org/publicdomain/zero/1.0/deed

extern crate vlc;

use std::{fs, thread};
use std::env::current_dir;
use std::fs::{File};
use std::io::{Read};



use std::sync::mpsc::channel;
use multiparty::server::sans_io::FormData;
use tera::{Context, Tera};

use tiny_http::{Server, Method, Header, StatusCode, Response};


use vlc::{Instance, Media, MediaPlayer};
use crate::Command::{HOME, PAUSE, PLAY};



#[derive(Debug)]
enum Command {
    PLAY,
    PAUSE,
    HOME
}


fn main() {
    let server = Server::http("0.0.0.0:8000").unwrap();

    let project_dir = current_dir().unwrap();




    let (command_tx, command_rx) = channel::<Command>();

    let _tx = command_tx.clone();
    let video_files_dir = project_dir.join("files");

    thread::spawn(move || {
        let instance = Instance::new().unwrap();

        //let md = Media::new_path(&instance, video_files_dir.as_path().join("Flight Footage.mp4")).unwrap();
        let home = Media::new_path(&instance, video_files_dir.as_path().join("k0rILCL.jpg")).unwrap();


/*        let em = md.event_manager();
        let _ = em.attach(EventType::MediaStateChanged, move |e, _| {
            match e {
                Event::MediaStateChanged(s) => {
                    println!("State : {:?}", s);
                    if s == State::Ended || s == State::Error {
                        tx.send(HOME).unwrap();
                    }
                },
                _ => (),
            }
        });*/

        let mdp = MediaPlayer::new(&instance).unwrap();
        mdp.set_media(&home);
        mdp.play().unwrap();

        while let Ok(command) = command_rx.recv() {
            match command {
                PLAY => {mdp.play().unwrap()}
                PAUSE => {mdp.set_pause(true)}
                HOME => {
                    mdp.set_media(&home);
                    mdp.play().unwrap();
                }
            };
            println!("Command Rx: {:?}", command);
        }


    });

    let mut tera = Tera::default();


    for request in server.incoming_requests() {
        println!("received request! method: {:?}, url: {:?}, headers: {:?}",
                 request.method(),
                 request.url(),
                 request.headers()
        );
        match request.method() {
            Method::Get => {
                if request.url() == "/play" {
                    command_tx.send(PLAY).unwrap();
                }else if request.url() == "/pause" {
                    command_tx.send(PAUSE).unwrap();
                }else {
                    //let path = PathBuf::from(filedir.as_path().join("Flight Footage.mp4"));
                    //mdp.set_media(&md);
                    //mdp.play().unwrap();

                }
            },
            Method::Post => {
                match request.url() {
                    "/upload" => {
                        if let Some(content_type) = request.headers().iter().find(|x| x.field.as_str() == "Content-Type") {
                            if content_type.value.as_str().contains("multipart/form-data") {
                                let boundary = content_type.value
                                    .as_str()
                                    .split("boundary=")
                                    .nth(1)
                                    .unwrap()
                                    .to_string();
                                println!("{}",boundary);

                                let _multipart = FormData::new(&boundary);


                            }
                        }

                    },
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


