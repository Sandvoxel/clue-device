use std::env::current_dir;
use std::fs;

use local_ip_address::local_ip;
use log::{error, info};
use vlc::{Instance, Media};
use crate::vlc_handler::image_generation::generate_image_with_text;

pub fn create_startup_file(instance: &Instance) -> Media {
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

    Media::new_path(instance, startup_image_location.as_path())
        .unwrap_or_else(||{
            error!("Failed to open Startup image at startup this is fatal closing app...");
            panic!("Failed to load Startup image")
        })
}

pub fn create_idle_image(instance: &Instance) -> Media {
    let idle_image_path = current_dir().unwrap().join("files").join("idle.png");

    if !idle_image_path.is_file() {
        let my_local_ip = local_ip().unwrap_or_else(|e|{
            let error = format!("Failed to find current IP address are you connected to the internet? : {:?}", e);
            error!("{}", error);
            panic!("{}", error);
        });

        match generate_image_with_text(
            format!("This is the default Idle screen to add your own upload one to {} with the name idle.png", my_local_ip).as_str(),
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

    return Media::new_path(instance, idle_image_path.as_path())
        .unwrap_or_else(||{
            error!("Failed to open Idle image at startup this is fatal closing app...");
            panic!("Failed to load idle image")
        })
}

pub fn create_paircard_image(instance: &Instance) -> Media {
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

    Media::new_path(instance, pair_card_image.as_path())
        .unwrap_or_else(||{
            error!("Failed to open Idle image at startup this is fatal closing app...");
            panic!("Failed to load idle image")
        })


}

