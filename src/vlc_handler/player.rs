

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::{fs, thread};
use std::env::current_dir;
use std::time::Duration;

use log::{error, info};
use vlc::{Instance, Media, MediaPlayer};
use crate::vlc_handler::default_images::{create_idle_image, create_paircard_image, create_startup_file};
use crate::vlc_handler::vlc_manager::{Command};


pub struct Player {
    media_player: MediaPlayer,
    idle_media: Media,
    pair_card_media: Media,
    command_channel: (Sender<Command>, Receiver<Command>),
}

impl Player {
    pub fn new(command_channel: (Sender<Command>, Receiver<Command>)) -> Player {
        if let Some(instance) = Instance::new(){
            if let Some(media_player) = MediaPlayer::new(&instance){

                let files_dir = current_dir().unwrap().join("files");

                if !files_dir.is_dir() {
                    info!("Creating Dir to store files at this location: {}", files_dir.as_path().display());
                    fs::create_dir(&files_dir).unwrap_or_else(|e|{
                        error!("Failed to create dir to store files: {:?}", e);
                        panic!("Could not create dir to store files: {:?}", e);
                    });
                }

                let idle_media = create_idle_image(&instance);

                let pair_card_media = create_paircard_image(&instance);

                let startup_media = create_startup_file(&instance);
                media_player.set_media(&startup_media);

                let tx = command_channel.0.clone();
                thread::spawn(move ||{
                    thread::sleep(Duration::from_secs(45));
                    tx.send(Command::Idle).unwrap_or_else(|e|{
                        error!("Failed to send Home Command to vlc player: {:?}", e);
                        panic!();
                    });
                });

                if let Ok(..) = media_player.play(){
                    return Player {
                        media_player,
                        idle_media,
                        pair_card_media,
                        command_channel,
                    }
                }
                error!("Vlc failed to play the idle image at start startup this is non recoverable closing.");
                panic!()
            }
            panic!();
        }
        panic!();
    }
}

impl Player {
    pub fn thread(&mut self) {
        while let Ok(command) = self.command_channel.1.recv() {
            info!("Vlc Received Command: {:?}", command);
            match command {
                Command::Play => {self.media_player.play().unwrap()}
                Command::Pause => {self.media_player.set_pause(true)}
                Command::Idle => {
                    self.media_player.set_media(&self.idle_media);
                    self.media_player.play().unwrap();
                }
                Command::PairCard(rec) => {
                    self.media_player.set_media(&self.pair_card_media);
                    self.media_player.play().unwrap();

                    let no_input = Arc::new(AtomicBool::new(true));

                    let no_input_clone = no_input.clone();

                    let tx = self.command_channel.0.clone();
                    thread::spawn(move ||{
                        thread::sleep(Duration::from_secs(300));
                        if no_input_clone.load(Ordering::SeqCst) {
                            tx.send(Command::Idle).unwrap_or_else(|e|{
                                error!("Failed to send Home Command to vlc player: {:?}", e);
                                panic!();
                            });
                        }
                    });


                    if rec.recv().is_ok(){
                        no_input.store(false, Ordering::SeqCst);

                    }
                }
            }
        }
    }
}


