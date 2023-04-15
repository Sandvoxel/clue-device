use std::env::current_dir;
use std::fs;
use std::sync::mpsc::Sender;
use log::{error, info};
use mfrc522::Mfrc522;
use sled::Db;
use crate::video_handler::media_manager::Command;

pub struct Rfid {
    vlc_command_channel: Sender<Command>,
    sled_database: Db
}

impl Rfid {
    pub fn pair_card(&self) {
        self.sled_database.insert("","").unwrap();
    }
}

impl Rfid {
    pub fn new(vlc_command_channel: Sender<Command>) -> Rfid {
        let database_dir = current_dir().unwrap().join("data");

        if !database_dir.is_dir() {
            info!("Creating Dir to for database at this location: {}", database_dir.as_path().display());
            fs::create_dir(&database_dir).unwrap_or_else(|e|{
                error!("Failed to create database dir: {:?}", e);
                panic!("Failed to create database dir: {:?}", e);
            });
        }

        let sled_database = sled::open(database_dir.join("card_database")).unwrap_or_else(|e|{
            error!("Failed to open database: {:?}", e);
            panic!("Failed to open database: {:?}", e);
        });


        Rfid {
            vlc_command_channel,
            sled_database
        }
    }
}

