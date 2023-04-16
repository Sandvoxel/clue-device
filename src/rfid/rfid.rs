use std::env::current_dir;
use std::{fs, thread};
use std::sync::mpsc::Sender;
use std::time::Duration;
use linux_embedded_hal::{Pin, Spidev};
use linux_embedded_hal::spidev::{SpidevOptions, SpiModeFlags};
use linux_embedded_hal::sysfs_gpio::Direction;
use embedded_hal::blocking::spi::{Transfer as SpiTransfer, Write as SpiWrite};
use anyhow::Result;


use log::{error, info};
use mfrc522::{Initialized, Mfrc522, WithNssDelay};
use sled::Db;
use crate::video_handler::media_manager::Command;
use crate::video_handler::media_manager::Command::PlayMedia;

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

    pub fn start_rfid_thread(&self) {

        let _tx = self.vlc_command_channel.clone();
        if is_raspberry_pi() {
            let tx = self.vlc_command_channel.clone();
            thread::spawn(move ||{
                let mut spi = Spidev::open("/dev/spidev0.0").unwrap();
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

                info!("VERSION: 0x{:x}", vers);

                assert!(vers == 0x91 || vers == 0x92);

                loop {
                    const CARD_UID: [u8; 4] = [34, 246, 178, 171];
                    const TAG_UID: [u8; 4] = [128, 170, 179, 76];

                    if let Ok(atqa) = mfrc522.reqa() {
                        info!("Test");
                        if let Ok(uid) = mfrc522.select(&atqa) {
                            info!("UID: {:?}", uid.as_bytes());

                            if uid.as_bytes() == &CARD_UID {
                                info!("CARD");
                            } else if uid.as_bytes() == &TAG_UID {
                                info!("TAG");
                            }

                            let media = current_dir().unwrap().join("files/Img 4541.mp4");

                            tx.send(PlayMedia(media)).unwrap();

                            handle_authenticate(&mut mfrc522, &uid, |m| {
                                let data = m.mf_read(1).unwrap_or_else(|err|{
                                    error!("Failed to read card: {:?}", err);
                                    [0; 16]
                                });
                                info!("read {:?}", data);
                                Ok(())
                            }).ok();
                            info!("Card read waiting 60s");
                            thread::sleep(Duration::from_secs(5));
                            info!("Finished waiting");

                        }
                    }

                    thread::sleep(Duration::from_millis(250));
                }

            });
        } else {
            error!("Not a raspberry pi not starting rfid reader");
        }
    }



}
fn handle_authenticate<E, SPI, NSS, D, F>(
    mfrc522: &mut Mfrc522<SPI, NSS, D, Initialized>,
    uid: &mfrc522::Uid,
    action: F,
) -> Result<()>
    where
        SPI: SpiTransfer<u8, Error = E> + SpiWrite<u8, Error = E>,
        Mfrc522<SPI, NSS, D, Initialized>: WithNssDelay,
        F: FnOnce(&mut Mfrc522<SPI, NSS, D, Initialized>) -> Result<()>,
        E: std::fmt::Debug + Sync + Send + 'static,
{
    // Use *default* key, this should work on new/empty cards
    let key = [0xFF; 6];
    if mfrc522.mf_authenticate(uid, 1, &key).is_ok() {
        action(mfrc522)?;
    } else {
        error!("Could not authenticate");
    }

    mfrc522.hlta().unwrap_or_else(|err|{
        error!("Failed rfid: {:?}", err);
    });
    mfrc522.stop_crypto1().unwrap_or_else(|err|{
        error!("Failed rfid: {:?}", err);
    });
    Ok(())
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
            hardware.contains("BCM2708")
                || hardware.contains("BCM2709")
                || hardware.contains("BCM2710")
                || hardware.contains("BCM2835")
        });

    is_raspberry_pi
}

#[cfg(not(all(target_os = "linux", target_arch = "arm")))]
fn is_raspberry_pi() -> bool {
    false
}

