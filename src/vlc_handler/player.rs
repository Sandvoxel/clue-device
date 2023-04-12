

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::{fs, thread};
use std::env::current_dir;
use std::path::PathBuf;
use std::ptr::null;
use std::time::Duration;

use log::{debug, error, info, warn};
use vlc::{Event, EventType, Instance, LogLevel, Media, MediaLibrary, MediaList, MediaPlayer, State};
use crate::vlc_handler::default_images::{create_idle_image, create_paircard_image, create_startup_file};
use crate::vlc_handler::vlc_manager::{Command};
use crate::vlc_handler::vlc_manager::Command::{Idle, Play, Pause, PairCard, PlayMedia};


pub struct Player {
    media_player: MediaPlayer,
    vlc_instance: Instance,
    idle_media: Media,
    pair_card_media: Media,
    command_channel: (Sender<Command>, Receiver<Command>),
}

impl Player {
    pub fn new(command_channel: (Sender<Command>, Receiver<Command>)) -> Player {
        if let Some(vlc_instance) = Instance::new(){
            vlc_instance.set_log(|loglevel, log, test|{
                match loglevel {
                    LogLevel::Debug => {debug!("Vlc: {}", test)}
                    LogLevel::Notice => {info!("Vlc: {}", test)}
                    LogLevel::Warning => {warn!("Vlc: {}", test)}
                    LogLevel::Error => {error!("Vlc: {}", test)}
                }
            });
            if let Some(media_player) =  MediaPlayer::new(&vlc_instance){

                let files_dir = current_dir().unwrap().join("files");

                if !files_dir.is_dir() {
                    info!("Creating Dir to store files at this location: {}", files_dir.as_path().display());
                    fs::create_dir(&files_dir).unwrap_or_else(|e|{
                        error!("Failed to create dir to store files: {:?}", e);
                        panic!("Could not create dir to store files: {:?}", e);
                    });
                }
                let idle_media = create_idle_image(&vlc_instance);

                let pair_card_media = create_paircard_image(&vlc_instance);

                let startup_media = create_startup_file(&vlc_instance);
                media_player.set_media(&startup_media);

                let tx = command_channel.0.clone();
                thread::spawn(move ||{
                    thread::sleep(Duration::from_secs(45));
                    tx.send(Idle).unwrap_or_else(|e|{
                        error!("Failed to send Home Command to vlc player: {:?}", e);
                        panic!();
                    });
                });

                if let Ok(..) = media_player.play(){
                    return Player {
                        media_player,
                        vlc_instance,
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
                Play => {self.media_player.play().unwrap()}
                Pause => {self.media_player.set_pause(true)}
                Idle => {
                    self.media_player.set_media(&self.idle_media);
                    self.media_player.play().unwrap();
                }
                PlayMedia(path) => {
                    if !is_playable_by_vlc(&path) {
                        error!("This media is not playable by vlc: {}", path.as_path().display());
                        return;
                    }
                    let media = if let Some(md) = Media::new_path(&self.vlc_instance, path) {
                        md
                    } else {
                        error!("Failed to create media instance");
                        return;
                    };


                    let tx = self.command_channel.0.clone();
                    let em = media.event_manager();
                    let _ = em.attach(EventType::MediaStateChanged, move |e, _| {
                        match e {
                            Event::MediaStateChanged(s) => {
                                println!("State : {:?}", s);
                                if s == State::Ended || s == State::Error {
                                    tx.send(Idle).unwrap();
                                    info!("Video Complete sending idle command to reset for next clue")
                                }
                            },
                            _ => (),
                        }
                    });


                    self.media_player.set_media(&media);
                    self.media_player.play().unwrap_or_else(|_|{
                        error!("Vlc failed to play the media");
                        if let Err(send_err) = self.command_channel.0.send(Idle) {
                            error!("Failed to send command to media player: {:?}", send_err)
                        }

                        return;
                    });

                }
                PairCard(rec) => {
                    self.media_player.set_media(&self.pair_card_media);
                    self.media_player.play().unwrap();

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


                    if rec.recv().is_ok(){
                        no_input.store(false, Ordering::SeqCst);
                    }
                }
            }
        }
    }
}

fn is_playable_by_vlc(file: &PathBuf) -> bool {
    let known_extensions = [
        "avi", "flv", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "ogg", "ogv", "webm", "wmv",
        "bmp", "gif", "jpeg", "jpg", "png", "tiff", "tif"
    ];

    match file.extension() {
        Some(ext) => known_extensions.contains(&ext.to_str().unwrap_or("").to_lowercase().as_str()),
        None => false,
    }
}



