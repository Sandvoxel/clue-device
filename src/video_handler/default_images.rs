use std::env::current_dir;
use std::fs;
use std::path::{PathBuf};

use local_ip_address::local_ip;
use log::{error, info};
use crate::video_handler::image_generation::generate_image_with_text;

pub fn create_startup_file() -> PathBuf {
    let files_dir = current_dir().unwrap().join("files");


    let startup_image_location = files_dir.join("startup.png");

    if startup_image_location.is_file() {
        fs::remove_file(&startup_image_location).unwrap_or_else(|e|{
            error!("Failed to remove Startup.png: {:?}", e);
            panic!();
        });
        info!("Removing startup image.");
    }

    if !startup_image_location.is_file() {
        let my_local_ip = local_ip().unwrap_or_else(|e|{
            let error = format!("Failed to find current IP address are you connected to the internet? : {:?}", e);
            error!("{}", error);
            panic!("{}", error);
        });

        match generate_image_with_text(format!("{}:8000",my_local_ip).as_str(), &startup_image_location) {
            Ok(_) => {
                info!("Created startup image");
            }
            Err(error) => {
                error!("{:?}", error);
                panic!();
            }
        };
    };

    startup_image_location.as_path().to_path_buf()
}

pub fn create_idle_image() -> PathBuf {
    let idle_image_path = current_dir().unwrap().join("files").join("idle.png");

    if !idle_image_path.is_file() {
        let my_local_ip = local_ip().unwrap_or_else(|e|{
            let error = format!("Failed to find current IP address are you connected to the internet? : {:?}", e);
            error!("{}", error);
            panic!("{}", error);
        });

        match generate_image_with_text(
            format!("This is the default Idle screen to add your own upload one to {}:8000 with the name idle.png", my_local_ip).as_str(),
                                       &idle_image_path) {
            Ok(_) => {
                info!("Created default idle image ");
            }
            Err(error) => {
                error!("{:?}", error);
                panic!();
            }
        };
    }

    idle_image_path
}

pub fn create_paircard_image() -> PathBuf {
    let pair_card_image = current_dir().unwrap().join("files").join("paircard.png");

    if !pair_card_image.is_file() {
        match generate_image_with_text("Tap card to reader to pair video", &pair_card_image) {
            Ok(_) => {
                info!("Created pair card image");
            }
            Err(error) => {
                error!("{:?}", error);
                panic!();
            }
        };
    }

    pair_card_image
}

