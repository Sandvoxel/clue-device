use std::env::current_dir;
use crate::vlc_handler::player::{Player};
use std::sync::mpsc::{channel, Receiver, Sender, SendError};
use std::{fs, thread};

use std::thread::JoinHandle;
use std::time::Duration;
use local_ip_address::local_ip;
use log::{error, info};

use crate::utils::image_generation::generate_image_with_text;
use crate::vlc_handler::vlc_manager::Command::Home;


#[derive(Debug)]
pub enum Command {
    Play,
    Pause,
    Home,
    PairCard(Receiver<()>)
}

pub struct VlcManager {
    command_channel: Sender<Command>,
    _player_thread_handle: JoinHandle<()>
}

impl VlcManager {
    pub fn new() -> VlcManager{
        let (command_tx, command_rx) = channel::<Command>();

        VlcManager {
            command_channel: command_tx.clone(),
            _player_thread_handle: thread::spawn(move || {
                Player::new((command_tx, command_rx))
                    .thread();
            })
        }

    }
}

impl VlcManager {
    pub fn send_command(&self, command: Command) -> Result<(), SendError<Command>> {
        self.command_channel.send(command)
    }
}