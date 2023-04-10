// This file is an example for vlc-rs, licensed under CC0.
// https://creativecommons.org/publicdomain/zero/1.0/deed

extern crate vlc;

use std::{fs, io};
use std::env::current_dir;

use std::fs::{File, read};
use std::io::{Read};
use std::path::{PathBuf};
use std::sync::mpsc::channel;

use multipart::server::{Multipart};
use tera::{Context, Tera};
use tiny_http::{Server, Method, Header, StatusCode, Response};
use image::{ColorType, ImageBuffer, Rgba};

use rusttype::{point, Scale};


use crate::Command::{PAUSE, PLAY};



#[derive(Debug)]
enum Command {
    PLAY,
    PAUSE,
    //HOME
}

#[cfg(target_os = "windows")]
fn default_font_path() -> PathBuf {
    PathBuf::from(r"C:\Windows\Fonts\arial.ttf")
}

#[cfg(target_os = "macos")]
fn default_font_path() -> PathBuf {
    PathBuf::from("/System/Library/Fonts/Supplemental/Arial.ttf")
}

#[cfg(target_os = "linux")]
fn default_font_path() -> PathBuf {
    PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
}

fn generate_image_with_text(text: &str, output_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Create an image buffer
    let mut image = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(1920, 1080);

    // Set the background color
    let background_color = Rgba([255, 255, 255, 255]);
    for pixel in image.pixels_mut() {
        *pixel = background_color;
    }

    // Draw text on the image
    let font = rusttype::Font::try_from_vec(read(default_font_path()).unwrap()).unwrap();

    let font_size = 50.0;

    let scale = Scale {
        x: 100.0,
        y: 100.0
    };
    // Draw text on the image
    let glyphs: Vec<rusttype::PositionedGlyph> = font.layout(text, scale, point(0.0, font_size)).collect();

    for glyph in glyphs {
        if let Some(bb) = glyph.pixel_bounding_box() {
            glyph.draw(|x, y, v| {
                let px = x as i32 + bb.min.x + 400;
                let py = y as i32 + bb.min.y + 400;

                if px >= 0 && px < image.width() as i32 && py >= 0 && py < image.height() as i32 {
                    let background_color = image.get_pixel(px as u32, py as u32);
                    let alpha = (v * 255.0) / 255.0;
                    let color = Rgba([
                        (background_color[0] as f32 * (1.0 - alpha) + 0.0 * alpha) as u8,
                        (background_color[1] as f32 * (1.0 - alpha) + 0.0 * alpha) as u8,
                        (background_color[2] as f32 * (1.0 - alpha) + 0.0 * alpha) as u8,
                        255,
                    ]);
                    image.put_pixel(px as u32, py as u32, color);
                }
            });
        }
    }

    // Save the image
    image::save_buffer(output_path, &image.clone().into_raw(), image.width(), image.height(), ColorType::Rgba8)?;

    Ok(())
}


fn main() {
    let server = Server::http("0.0.0.0:8000").unwrap();

    let project_dir = current_dir().unwrap();

    if fs::remove_file(project_dir.join("files").join("idle.png")).is_ok() {
        println!("Deleteing idle.png");
    }



    if !project_dir.join("files").join("idle.png").is_file() {
        match generate_image_with_text("Idle PNG", project_dir.join("files/idle.png")) {
            Ok(_) => {
                println!("Created File!")
            }
            Err(error) => {
                println!("{:?}", error)
            }
        };
    }



    let (command_tx, _command_rx) = channel::<Command>();

    let _tx = command_tx.clone();
    let _video_files_dir = project_dir.join("files");

/*    thread::spawn(move || {
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


    });*/

    let mut tera = Tera::default();


    for mut request in server.incoming_requests() {
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

                            println!("{}", file_path.as_path().to_str().unwrap());

                            let mut file = File::create(&file_path).unwrap();
                            io::copy(&mut field.data, &mut file).unwrap();
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


