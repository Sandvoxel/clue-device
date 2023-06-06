

use std::sync::{Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::{fs, thread};
use std::env::current_dir;


use std::path::{Path, PathBuf};

use std::time::Duration;
use libmpv::{FileState, Mpv};

use log::{error, info};
use crate::video_handler::default_images::{create_idle_image, create_paircard_image, create_startup_file};
use crate::video_handler::media_manager::{Command};
use crate::video_handler::media_manager::Command::{Idle, PairCard, PlayMedia};


pub struct Player{
    media_player: Mpv,
    idle_media: PathBuf,
    pair_card_media: PathBuf,
    command_channel: (Sender<Command>, Receiver<Command>),
}

impl Player {
    //FIXME use proper error here
    pub fn new(command_channel: (Sender<Command>, Receiver<Command>)) -> Result<Player, libmpv::Error> {
        if let Ok(media_player) = Mpv::new() {
            media_player.set_property("volume", 100)?;
            media_player.set_property("keep-open", "yes")?;
            media_player.set_property("ao", "alsa")?;


            let files_dir = current_dir().unwrap().join("files");

            if !files_dir.is_dir() {
                info!("Creating Dir to store files at this location: {}", files_dir.as_path().display());
                fs::create_dir(&files_dir).unwrap_or_else(|e|{
                    error!("Failed to create dir to store files: {:?}", e);
                    panic!("Could not create dir to store files: {:?}", e);
                });
            }
            let idle_media = create_idle_image();

            let pair_card_media = create_paircard_image();

            let startup_media = create_startup_file();

            media_player.playlist_load_files(&[(startup_media.as_path().display().to_string().as_str(), FileState::Replace, None)]).unwrap();

            let tx = command_channel.0.clone();
            thread::spawn(move ||{
                thread::sleep(Duration::from_secs(45));
                tx.send(Idle).unwrap_or_else(|e|{
                    error!("Failed to send Home Command to vlc player: {:?}", e);
                    panic!();
                });
            });

            return Ok(Player {
                media_player,
                idle_media,
                pair_card_media,
                command_channel,
            });

        }
        Err(libmpv::Error::Null)
    }

}



impl Player {


    pub fn thread(&mut self) {
        while let Ok(command) = self.command_channel.1.recv() {
            info!("Media Player Received Command: {:?}", command);
            match command {
                Idle => {
                    //FIXME: need to not crash here
                    self.media_player.playlist_load_files(&[(self.idle_media.as_path().display().to_string().as_str(), FileState::Replace, None)])
                        .unwrap();
                }
                PlayMedia(path) => {
                    if is_playable_by_mpv(path.as_path()) {
                        info!("Playing: {}", path.display());
                        self.media_player.playlist_load_files(&[(path.as_path().display().to_string().as_str(), FileState::Replace, None)])
                            .unwrap_or_else(|| {
                                self.media_player = Mpv::new().unwrap();
                                self.media_player.playlist_load_files(&[(path.as_path().display().to_string().as_str(), FileState::Replace, None)]).unwrap();
                            });
                        self.media_player.unpause().unwrap();
                        self.media_player.playlist_load_files(&[(self.idle_media.as_path().display().to_string().as_str(), FileState::AppendPlay, None)])
                            .unwrap();
                    }else {
                        error!("File \"{}\" is not playable", path.display())
                    }

                }
                PairCard => {

                    let no_input = Arc::new(AtomicBool::new(true));

                    let no_input_clone = no_input.clone();

                    let tx = self.command_channel.0.clone();
                    thread::spawn(move ||{
                        thread::sleep(Duration::from_secs(300));
                        if no_input_clone.load(Ordering::SeqCst) {
                            tx.send(Idle).unwrap_or_else(|e|{
                                error!("Failed to send Idle Command to vlc player: {:?}", e);
                                panic!();
                            });
                        }
                    });
                    self.media_player.playlist_load_files(&[(self.pair_card_media.as_path().display().to_string().as_str(), FileState::Replace, None)])
                        .unwrap();
                    no_input.store(false, Ordering::SeqCst);
                }
            }
        }
    }
}

fn is_playable_by_mpv(file: &Path) -> bool {
    let known_extensions = [
        "mp4", "jpeg", "jpg", "png"
    ];

    match file.extension() {
        Some(ext) => known_extensions.contains(&ext.to_str().unwrap_or("").to_lowercase().as_str()),
        None => false,
    }
}



