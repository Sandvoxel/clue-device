use crate::video_handler::player::{Player};
use std::sync::mpsc::{channel, Sender, SendError};
use std::{thread};
use std::path::PathBuf;

use std::thread::JoinHandle;



#[derive(Debug)]
pub enum Command {
    Idle,
    PlayMedia(PathBuf),
    PairCard,
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
                    .expect("FIXME: this should be changed")
                    .thread();
            })
        }

    }
}

impl VlcManager {
    pub fn send_command(&self, command: Command) -> Result<(), SendError<Command>> {
        self.command_channel.send(command)
    }

    pub fn get_command_channel(&self) -> Sender<Command> {
        self.command_channel.clone()
    }
}