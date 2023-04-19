use std::env::current_dir;
use std::{fs, thread};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::mpsc::{channel, Receiver, Sender, SendError, TryRecvError};
use std::time::Duration;
use linux_embedded_hal::{Pin, Spidev};
use linux_embedded_hal::spidev::{SpidevOptions, SpiModeFlags};
use linux_embedded_hal::sysfs_gpio::Direction;
use embedded_hal::blocking::spi::{Transfer as SpiTransfer, Write as SpiWrite};
use anyhow::Result;

use log::{error, info};
use mfrc522::{Initialized, Mfrc522, WithNssDelay};
use mfrc522::error::Error;
use serde::de::Unexpected::Str;
use sled::{Db, IVec};
use uuid::{Bytes, Uuid};
use crate::config::setup::DeviceConfiguration;
use crate::video_handler::media_manager::Command;
use crate::video_handler::media_manager::Command::{Idle, PlayMedia};

#[derive(Debug, Clone)]
enum RfidCommands {
    PairCard(PathBuf)
}

pub struct Rfid {
    vlc_command_channel: Sender<Command>,
    sled_database: Db,
    device_configuration: DeviceConfiguration,
    command_channel: Sender<RfidCommands>
}

impl Rfid {
    pub fn pair_card(&self, path: &Path){
        match self.vlc_command_channel.send(Command::PairCard) {
            Ok(_) => {
                info!("Sent command to vlc to display pair screen");
                match self.command_channel.send(RfidCommands::PairCard(path.to_path_buf())) {
                    Ok(_) => {
                        info!("Sent command to rfid reader pair a card");
                    }
                    Err(err) => {
                        error!("Failed to send command to rfid reader: {:?}", err);
                        if let Err(err) = self.vlc_command_channel.send(Command::Idle) {
                            error!("Failed to send message to send vlc back to idle screen");
                        }
                    }
                }
            }
            Err(err) => {
                error!("Failed to send command to vlc: {:?}", err);
            }
        }

    }
}

impl Rfid {
    pub fn new(vlc_command_channel: Sender<Command>, device_configuration: DeviceConfiguration) -> Rfid {
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


        let commands = channel();
        let rfid = Rfid {
            vlc_command_channel,
            sled_database,
            device_configuration,
            command_channel: commands.0
        };

        rfid.start_rfid_thread(commands.1);
        rfid
    }

    fn start_rfid_thread(&self,commands_rx: Receiver<RfidCommands>) {
        if is_raspberry_pi() {
            let clue_timeout = self.device_configuration.clue_timeout;
            let tx = self.vlc_command_channel.clone();
            let database = self.sled_database.clone();
            let retrys = self.device_configuration.rfid_retrys;
            thread::spawn(move || {
                for i in 0..retrys {
                    info!("Starting rfid reader ({} of {})", i, retrys-1);
                    let spi = Spidev::open("/dev/spidev0.0");
                    if spi.is_err() {
                        error!("Failed to open spi device waiting 3 seconds then retrying");
                        thread::sleep(Duration::from_secs(3));
                        break;
                    }
                    let mut spi = spi.unwrap();

                    let options = SpidevOptions::new()
                        .max_speed_hz(1_000_000)
                        .mode(SpiModeFlags::SPI_MODE_0)
                        .build();
                    spi.configure(&options).unwrap();

                    // software-controlled chip select pin
                    let pin = Pin::new(22);
                    pin.export().unwrap();
                    while !pin.is_exported() {}
                    thread::sleep(Duration::from_millis(25));
                    pin.set_direction(Direction::Out).unwrap();
                    pin.set_value(1).unwrap();

                    // The `with_nss` method provides a GPIO pin to the driver for software controlled chip select.
                    let mut mfrc522 = Mfrc522::new(spi).with_nss(pin).init().unwrap_or_else(|err|{
                        error!("Failed to open rfid reader: {:?}", err);
                        panic!("This is non recoverable");
                    });

                    let vers = mfrc522.version().unwrap_or_else(|err|{
                        error!("Failed to read verion from rfid board: {:?}",err);
                        error!("Will try again: {:?}",err);
                        mfrc522.version().unwrap_or_else(|err|{
                            error!("Failed to read verion from rfid board x2 Fatal: {:?}",err);
                            panic!();
                        })
                    });


                    info!("Mfrc522 VERSION: 0x{:x}", vers);

                    assert!(vers == 0x91 || vers == 0x92);

                    loop {
                        match mfrc522.reqa() {
                            Ok(atqa) =>{
                                if let Ok(uid) = mfrc522.select(&atqa) {
                                    info!("UID: {:?}", uid.as_bytes());

                                    match commands_rx.try_recv() {
                                        Ok(message) => {
                                            match message { RfidCommands::PairCard(path) => {

                                                if let Ok(_) = database.insert(slice_to_uuid(uid.as_bytes()).to_string(), path.display().to_string().as_str()) {
                                                    info!("Card written waiting {}S",clue_timeout);
                                                    tx.send(Idle).unwrap_or_else(|err|{
                                                        error!("Failed send idle screen");
                                                    });
                                                    thread::sleep(Duration::from_secs(clue_timeout));
                                                }
                                            }}
                                        },
                                        Err(TryRecvError::Empty) => {

                                            if let Ok(Some(data)) = database.get(slice_to_uuid(uid.as_bytes()).to_string()) {
                                                let bytes: &[u8] = data.as_ref();
                                                if let Ok(path) = std::str::from_utf8(bytes){
                                                    let media = PathBuf::from(path);
                                                    let f = File::open(&media).unwrap();
                                                    let size = f.metadata().unwrap().len();
                                                    let reader = BufReader::new(f);

                                                    let mp4 = mp4::Mp4Reader::read_header(reader, size).unwrap();

                                                    tx.send(PlayMedia(media)).unwrap();
                                                    let wait = mp4.duration().as_secs() + clue_timeout;
                                                    info!("Card read waiting {}S",wait);
                                                    thread::sleep(Duration::from_secs(wait));
                                                    info!("Finished waiting");
                                                }
                                            } else {
                                                info!("No database entry found for card: {:?}", uid.as_bytes());
                                            }
                                        },
                                        Err(TryRecvError::Disconnected) => error!("Channel disconnected"),
                                    }
                                }
                            }
                            Err(Error::LostCommunication) => {
                                break;
                            }
                            _ => {}
                        }


                        if let Ok(atqa) = mfrc522.reqa() {

                        }

                        thread::sleep(Duration::from_millis(250));
                    }

                    error!("RFID communication lost waiting 5 seconds then restarting");
                    thread::sleep(Duration::from_secs(5));
                }
                error!("Rfid reader not found.");
            });
        } else {
            error!("Not a raspberry pi not starting rfid reader");
        }
    }
}

fn slice_to_uuid(data: &[u8]) -> Uuid {
    let array: [u8; 16] = data
        .iter()
        .cloned()
        .chain(std::iter::repeat(0))
        .take(16)
        .collect::<Vec<u8>>()
        .try_into()
        .unwrap_or_else(|_| {
            error!("Failed to convert bytes into correct format");
            [0; 16]
        });

    Uuid::from_bytes(Bytes::from(array))
}

#[cfg(all(target_os = "linux", target_arch = "arm"))]
fn is_raspberry_pi() -> bool {
    let cpuinfo = fs::read_to_string("/proc/cpuinfo").expect("Failed to read /proc/cpuinfo");

    let is_raspberry_pi = cpuinfo
        .lines()
        .filter_map(|line| {
            let mut parts = line.split(':');
            let key = parts.next()?.trim();
            let value = parts.next()?.trim();

            if key == "Hardware" {
                Some(value)
            } else {
                None
            }
        })
        .any(|hardware| {
            hardware.contains("BCM")
        });

    is_raspberry_pi
}

#[cfg(not(all(target_os = "linux", target_arch = "arm")))]
fn is_raspberry_pi() -> bool {
    false
}

