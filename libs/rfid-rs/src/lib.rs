//! Driver library for interfacing with the MFRC522 contacless communication IC,
//! based on the [embedded-hal](https://docs.rs/embedded-hal/latest/embedded_hal/) traits.
//!
//! The MFRC522 is a *Proximity Coupling Device* (PCD) and communicates with a
//! *Proximity Integrated Circuit Card* (PICC).
//! The main purpose of the MFRC522 is to give the connected device
//! (where we're running this driver) the ability to read and write data from/to the card.
//!
//! The MFRC522 supports 3 communication interfaces:
//! - SPI
//! - I2C
//! - UART
//!
//! However, currently only SPI communication is implemented in this crate.
//!
//! # Quickstart
//! ```rust
//! // create an SPI device that implements the embedded-hal `spi::Transfer` and `spi::Write` traits
//! let spi = spi::Spi::new(/* */);
//! // create a GPIO output for chip-select control
//! let cs = gpio::Output::new(/* */);
//! let mut mfrc522 = Mfrc522::new(spi).with_nss(cs).init()?;
//!
//! // The reported version is expected to be 0x91 or 0x92
//! let mfrc522_version = mfrc522.version()?;
//! ```
//!
//! Take a look at [Mfrc522] for information on the available functions.
//!
//! # Example applications
//!
//! - [Raspberry Pi 4](https://gitlab.com/jspngh/rfid-rs/-/blob/master/examples/rpi.rs)
//! - [Beaglebone Black](https://gitlab.com/jspngh/rfid-rs/-/blob/master/examples/beagle.rs)
//! - [STM32L4](https://gitlab.com/jspngh/stm32l4-mfrc522)

#![cfg_attr(not(feature = "std"), no_std)]

pub mod error;
mod picc;
mod register;
mod util;

use core::time::Duration;
use embedded_hal as hal;
use hal::blocking::spi;
use hal::digital::v2::OutputPin;

use heapless::Vec;

use error::Error;
use register::*;
use util::{DummyDelay, DummyNSS, Sealed};

const MIFARE_ACK: u8 = 0xA;
const MIFARE_KEYSIZE: usize = 6;
pub type MifareKey = [u8; MIFARE_KEYSIZE];

pub enum Uid {
    /// Single sized UID, 4 bytes long
    Single(GenericUid<4>),
    /// Double sized UID, 7 bytes long
    Double(GenericUid<7>),
    /// Trip sized UID, 10 bytes long
    Triple(GenericUid<10>),
}

impl Uid {
    pub fn as_bytes(&self) -> &[u8] {
        match &self {
            Uid::Single(u) => u.as_bytes(),
            Uid::Double(u) => u.as_bytes(),
            Uid::Triple(u) => u.as_bytes(),
        }
    }
}

pub struct GenericUid<const T: usize>
where
    [u8; T]: Sized,
{
    /// The UID can have 4, 7 or 10 bytes.
    bytes: [u8; T],
    /// The SAK (Select acknowledge) byte returned from the PICC after successful selection.
    sak: picc::Sak,
}

impl<const T: usize> GenericUid<T> {
    pub fn new(bytes: [u8; T], sak_byte: u8) -> Self {
        Self {
            bytes,
            sak: picc::Sak::from(sak_byte),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn is_compliant(&self) -> bool {
        self.sak.is_compliant()
    }
}

/// Answer To reQuest type A
pub struct AtqA {
    bytes: [u8; 2],
}

/// Implemented by the different states of the MFRC522 driver.
///
/// This trait cannot be implemented outside of this crate.
pub trait State: Sealed {}

/// The MFRC522 driver starts in this state and needs to be initialized before it can be used.
pub enum Uninitialized {}
/// The MFRC522 driver is ready for use.
pub enum Initialized {}

impl State for Uninitialized {}
impl State for Initialized {}
impl Sealed for Uninitialized {}
impl Sealed for Initialized {}

/// MFRC522 driver
pub struct Mfrc522<SPI, NSS, D, S: State> {
    spi: SPI,
    nss: NSS,
    delay: D,
    state: core::marker::PhantomData<S>,
}

impl<E, SPI> Mfrc522<SPI, DummyNSS, DummyDelay, Uninitialized>
where
    SPI: spi::Transfer<u8, Error = E> + spi::Write<u8, Error = E>,
{
    /// Create a new MFRC522 driver from a SPI interface.
    ///
    /// The resulting driver will use a *dummy* NSS pin and expects the
    /// actual chip-select to be controlled by hardware.
    ///
    /// Use the [with_nss](Mfrc522::with_nss) method to add a software controlled NSS pin.
    ///
    /// If you are using optimization / release mode, you may want to add a delay function
    /// using the [with_delay](Mfrc522::with_delay) method to ensure timing requirements are
    /// respected.
    pub fn new(spi: SPI) -> Mfrc522<SPI, DummyNSS, DummyDelay, Uninitialized> {
        Mfrc522 {
            spi,
            nss: DummyNSS {},
            delay: DummyDelay {},
            state: core::marker::PhantomData,
        }
    }
}

impl<SPI, D> Mfrc522<SPI, DummyNSS, D, Uninitialized> {
    /// Add a software controlled chip-select/NSS pin that should be used by this driver
    /// for the SPI communication.
    ///
    /// This is necessary if your hardware does not support hardware controller NSS
    /// or if it is unavailable to you for some reason.
    pub fn with_nss<NSS: OutputPin>(self, nss: NSS) -> Mfrc522<SPI, NSS, D, Uninitialized> {
        Mfrc522 {
            spi: self.spi,
            nss,
            delay: self.delay,
            state: core::marker::PhantomData,
        }
    }
}

impl<SPI, NSS> Mfrc522<SPI, NSS, DummyDelay, Uninitialized> {
    /// Add a delay function to be used after each SPI transaction.
    ///
    /// The MFRC522 specifies that the NSS pin needs to be high/de-asserted
    /// for at least 50ns between communications.
    ///
    /// If optimizations are enabled, we can run into issues with this timing requirement
    /// if we do not add a delay between transactions.
    /// This function allows the user to specify a (platform specific) function
    /// that will (busy) wait for at least 50ns.
    pub fn with_delay<D: FnMut()>(self, delay: D) -> Mfrc522<SPI, NSS, D, Uninitialized> {
        Mfrc522 {
            spi: self.spi,
            nss: self.nss,
            delay,
            state: core::marker::PhantomData,
        }
    }
}

impl<SPI, NSS, D, S: State> Mfrc522<SPI, NSS, D, S> {
    /// Release the underlying SPI device and NSS pin
    pub fn release(self) -> (SPI, NSS) {
        (self.spi, self.nss)
    }
}

// The driver can transition to the `Initialized` state using this function
impl<E, SPI, NSS, D> Mfrc522<SPI, NSS, D, Uninitialized>
where
    SPI: spi::Transfer<u8, Error = E> + spi::Write<u8, Error = E>,
    Mfrc522<SPI, NSS, D, Uninitialized>: WithNssDelay,
{
    /// Initialize the MFRC522.
    ///
    /// This needs to be called before you can do any other operation.
    pub fn init(mut self) -> Result<Mfrc522<SPI, NSS, D, Initialized>, E> {
        self.reset()?;
        self.write(Register::TxModeReg, 0x00)?;
        self.write(Register::RxModeReg, 0x00)?;
        // Reset ModWidthReg to default value
        self.write(Register::ModWidthReg, 0x26)?;

        // Configure the timer, so we can get a timeout if something goes wrong
        // when communicating with a PICC:
        // - Set timer to start automatically at the end of the transmission
        self.write(Register::TModeReg, 0x80)?;
        // - Configure the prescaler to determine the timer frequency:
        //   f_timer = 13.56 MHz / (2 * TPreScaler + 1)
        //   so for 40kHz frequency (25μs period), TPreScaler = 0x0A9
        self.write(Register::TPrescalerReg, 0xA9)?;
        // - Set the reload value to determine the timeout
        //   for a 25ms timeout, we need a value of 1000 = 0x3E8
        self.write(Register::TReloadRegHigh, 0x03)?;
        self.write(Register::TReloadRegLow, 0xE8)?;

        // TODO: may not be necessary?
        self.write(Register::TxASKReg, FORCE_100_ASK)?;
        // Set preset value of CRC coprocessor according to ISO 14443-3 part 6.2.4
        self.write(Register::ModeReg, (0x3f & (!0b11)) | 0b01)?;
        // Enable antenna
        self.rmw(Register::TxControlReg, |b| b | 0b11)?;

        Ok(Mfrc522 {
            spi: self.spi,
            nss: self.nss,
            delay: self.delay,
            state: core::marker::PhantomData,
        })
    }
}

// The public functions can only be used after initializing
impl<E, SPI, NSS, D> Mfrc522<SPI, NSS, D, Initialized>
where
    SPI: spi::Transfer<u8, Error = E> + spi::Write<u8, Error = E>,
    Mfrc522<SPI, NSS, D, Initialized>: WithNssDelay,
{
    /// Sends a REQuest type A to nearby PICCs
    pub fn reqa(&mut self) -> Result<AtqA, Error<E>> {
        // NOTE REQA is a short frame (7 bits)
        let fifo_data = self.transceive(&[picc::Command::ReqA as u8], 7, 0)?;
        if fifo_data.valid_bytes != 2 || fifo_data.valid_bits != 0 {
            Err(Error::IncompleteFrame)
        } else {
            Ok(AtqA {
                bytes: fifo_data.buffer,
            })
        }
    }

    /// Sends a Wake UP type A to nearby PICCs
    pub fn wupa(&mut self) -> Result<AtqA, Error<E>> {
        // NOTE WUPA is a short frame (7 bits)
        let fifo_data = self.transceive(&[picc::Command::WupA as u8], 7, 0)?;
        if fifo_data.valid_bytes != 2 || fifo_data.valid_bits != 0 {
            Err(Error::IncompleteFrame)
        } else {
            Ok(AtqA {
                bytes: fifo_data.buffer,
            })
        }
    }

    /// Sends command to enter HALT state
    pub fn hlta(&mut self) -> Result<(), Error<E>> {
        let mut buffer: [u8; 4] = [picc::Command::HltA as u8, 0, 0, 0];
        let crc = self.calculate_crc(&buffer[..2])?;
        buffer[2..].copy_from_slice(&crc);

        // The standard says:
        //   If the PICC responds with any modulation during a period of 1 ms
        //   after the end of the frame containing the HLTA command,
        //   this response shall be interpreted as 'not acknowledge'.
        // We interpret that this way: only Error::Timeout is a success.
        match self.transceive::<0>(&buffer, 0, 0) {
            Err(Error::Timeout) => Ok(()),
            Ok(_) => Err(Error::Nak),
            Err(e) => Err(e),
        }
    }

    /// Selects a PICC in the READY state
    // TODO add optional UID to select a specific PICC
    pub fn select(&mut self, atqa: &AtqA) -> Result<Uid, Error<E>> {
        // check for proprietary anticollision
        if (atqa.bytes[0] & 0b00011111).count_ones() != 1 {
            return Err(Error::Proprietary);
        }

        // clear `ValuesAfterColl`
        self.rmw(Register::CollReg, |b| b & !0x80)
            .map_err(Error::Spi)?;

        let mut cascade_level: u8 = 0;
        let mut uid_bytes: [u8; 10] = [0u8; 10];
        let mut uid_idx: usize = 0;

        let sak = 'cascade: loop {
            let cmd = match cascade_level {
                0 => picc::Command::SelCl1,
                1 => picc::Command::SelCl2,
                2 => picc::Command::SelCl3,
                _ => unreachable!(),
            };
            let mut known_bits = 0;
            let mut tx = [0u8; 9];
            tx[0] = cmd as u8;

            // TODO: limit to 32 iterations (as spec dictates)
            'anticollision: loop {
                let tx_last_bits = known_bits % 8;
                let tx_bytes = 2 + known_bits / 8;
                let end = tx_bytes as usize + if tx_last_bits > 0 { 1 } else { 0 };
                tx[1] = (tx_bytes << 4) + tx_last_bits;

                // Tell transceive the only send `tx_last_bits` of the last byte
                // and also to put the first received bit at location `tx_last_bits`.
                // This makes it easier to append the received bits to the uid (in `tx`).
                match self.transceive::<5>(&tx[0..end], tx_last_bits, tx_last_bits) {
                    Ok(fifo_data) => {
                        fifo_data.copy_bits_to(&mut tx[2..=6], known_bits);
                        break 'anticollision;
                    }
                    Err(Error::Collision) => {
                        let coll_reg = self.read(Register::CollReg).map_err(Error::Spi)?;
                        if coll_reg & (1 << 5) != 0 {
                            // CollPosNotValid
                            return Err(Error::Collision);
                        }
                        let mut coll_pos = coll_reg & 0x1F;
                        if coll_pos == 0 {
                            coll_pos = 32;
                        }
                        if coll_pos < known_bits {
                            // No progress
                            return Err(Error::Collision);
                        }
                        let fifo_data = self.fifo_data::<5>()?;
                        fifo_data.copy_bits_to(&mut tx[2..=6], known_bits);
                        known_bits = coll_pos;

                        // Set the bit of collision position to 1
                        let count = known_bits % 8;
                        let check_bit = (known_bits - 1) % 8;
                        let index: usize =
                            1 + (known_bits / 8) as usize + if count != 0 { 1 } else { 0 };
                        tx[index] |= 1 << check_bit;
                    }
                    Err(e) => return Err(e),
                }
            }

            // send select
            tx[1] = 0x70; // NVB: 7 valid bytes
            tx[6] = tx[2] ^ tx[3] ^ tx[4] ^ tx[5]; // BCC

            let crc = self.calculate_crc(&tx[..7])?;
            tx[7..].copy_from_slice(&crc);

            let rx = self.transceive::<3>(&tx[0..9], 0, 0)?;
            if rx.valid_bytes != 3 || rx.valid_bits != 0 {
                return Err(Error::IncompleteFrame);
            }

            let sak = picc::Sak::from(rx.buffer[0]);
            let crc_a = &rx.buffer[1..];
            let crc_verify = self.calculate_crc(&rx.buffer[..1])?;
            if crc_a != crc_verify {
                return Err(Error::Crc);
            }

            if !sak.is_complete() {
                uid_bytes[uid_idx..uid_idx + 3].copy_from_slice(&tx[3..6]);
                uid_idx += 3;
                cascade_level += 1;
            } else {
                uid_bytes[uid_idx..uid_idx + 4].copy_from_slice(&tx[2..6]);
                break 'cascade sak;
            }
        };

        match cascade_level {
            0 => Ok(Uid::Single(GenericUid {
                bytes: uid_bytes[0..4].try_into().unwrap(),
                sak,
            })),
            1 => Ok(Uid::Double(GenericUid {
                bytes: uid_bytes[0..7].try_into().unwrap(),
                sak,
            })),
            2 => Ok(Uid::Triple(GenericUid {
                bytes: uid_bytes,
                sak,
            })),
            _ => unreachable!(),
        }
    }

    /// Switch off the MIFARE Crypto1 unit.
    /// Must be done after communication with an authenticated PICC
    pub fn stop_crypto1(&mut self) -> Result<(), Error<E>> {
        self.rmw(Register::Status2Reg, |b| b & !0x08)
            .map_err(Error::Spi)
    }

    pub fn mf_authenticate(
        &mut self,
        uid: &Uid,
        block: u8,
        key: &MifareKey,
    ) -> Result<(), Error<E>> {
        // stop any ongoing command
        self.command(Command::Idle).map_err(Error::Spi)?;
        // clear all interrupt flags
        self.write(Register::ComIrqReg, 0x7f).map_err(Error::Spi)?;
        // flush FIFO buffer
        self.fifo_flush().map_err(Error::Spi)?;
        // clear bit framing
        self.write(Register::BitFramingReg, 0).map_err(Error::Spi)?;

        let mut tx_buffer = [0u8; 12];
        tx_buffer[0] = picc::Command::MfAuthKeyA as u8;
        tx_buffer[1] = block;
        tx_buffer[2..8].copy_from_slice(key);
        match uid {
            Uid::Single(u) => tx_buffer[8..12].copy_from_slice(&u.bytes[0..4]),
            Uid::Double(u) => tx_buffer[8..12].copy_from_slice(&u.bytes[0..4]),
            Uid::Triple(u) => tx_buffer[8..12].copy_from_slice(&u.bytes[0..4]),
        };
        // write data to transmit to the FIFO buffer
        self.write_many(Register::FIFODataReg, &tx_buffer)?;

        // signal command
        self.command(Command::MFAuthent).map_err(Error::Spi)?;

        let mut timeout = 0;
        let mut irq;
        loop {
            irq = self.read(Register::ComIrqReg).map_err(Error::Spi)?;

            if irq & (ERR_IRQ | IDLE_IRQ) != 0 {
                break;
            } else if irq & TIMER_IRQ != 0 || timeout > 70 {
                return Err(Error::Timeout);
            }
            timeout = timeout + 1;
        }

        self.check_error_register()?;
        Ok(())
    }

    pub fn mf_read(&mut self, block: u8) -> Result<[u8; 16], Error<E>> {
        let mut tx = [picc::Command::MfRead as u8, block, 0u8, 0u8];

        let crc = self.calculate_crc(&tx[0..2])?;
        tx[2..].copy_from_slice(&crc);

        let rx = self.transceive::<18>(&tx, 0, 0)?.buffer;

        // verify CRC
        let crc = self.calculate_crc(&rx[..16])?;
        if crc != rx[16..] {
            return Err(Error::Crc);
        }
        Ok(rx[..16].try_into().unwrap())
    }

    pub fn mf_write(&mut self, block: u8, data: [u8; 16]) -> Result<(), Error<E>> {
        let mut cmd = [picc::Command::MfWrite as u8, block, 0, 0];
        let crc = self.calculate_crc(&cmd[0..2])?;
        cmd[2..].copy_from_slice(&crc);
        let fifo_data = self.transceive::<1>(&cmd, 0, 0)?;
        if fifo_data.valid_bytes != 1 || fifo_data.valid_bits != 4 {
            return Err(Error::Nak);
        }

        let mut tx = [0u8; 18];
        let crc = self.calculate_crc(&data)?;
        tx[..16].copy_from_slice(&data);
        tx[16..].copy_from_slice(&crc);
        let fifo_data = self.transceive::<1>(&tx, 0, 0)?;
        if fifo_data.valid_bytes != 1 || fifo_data.valid_bits != 4 {
            return Err(Error::Nak);
        }

        Ok(())
    }

    /// Returns the version reported by the MFRC522
    pub fn version(&mut self) -> Result<u8, Error<E>> {
        self.read(Register::VersionReg).map_err(Error::Spi)
    }

    pub fn new_card_present(&mut self) -> Result<AtqA, Error<E>> {
        self.write(Register::TxModeReg, 0x00).map_err(Error::Spi)?;
        self.write(Register::RxModeReg, 0x00).map_err(Error::Spi)?;
        self.write(Register::ModWidthReg, 0x26)
            .map_err(Error::Spi)?;

        self.reqa()
    }
}

// The private functions are implemented for all states.
impl<E, SPI, NSS, D, S: State> Mfrc522<SPI, NSS, D, S>
where
    SPI: spi::Transfer<u8, Error = E> + spi::Write<u8, Error = E>,
    Mfrc522<SPI, NSS, D, S>: WithNssDelay,
{
    fn calculate_crc(&mut self, data: &[u8]) -> Result<[u8; 2], Error<E>> {
        // stop any ongoing command
        self.command(Command::Idle).map_err(Error::Spi)?;

        // clear the CRC_IRQ interrupt flag
        self.write(Register::DivIrqReg, 1 << 2)
            .map_err(Error::Spi)?;

        // flush FIFO buffer
        self.fifo_flush().map_err(Error::Spi)?;

        // write data to transmit to the FIFO buffer
        self.write_many(Register::FIFODataReg, data)?;

        self.command(Command::CalcCRC).map_err(Error::Spi)?;

        // Wait for the CRC calculation to complete.
        let mut irq;
        for _ in 0..5000 {
            irq = self.read(Register::DivIrqReg).map_err(Error::Spi)?;

            if irq & CRC_IRQ != 0 {
                self.command(Command::Idle).map_err(Error::Spi)?;
                let crc = [
                    self.read(Register::CRCResultRegLow).map_err(Error::Spi)?,
                    self.read(Register::CRCResultRegHigh).map_err(Error::Spi)?,
                ];

                return Ok(crc);
            }
        }
        Err(Error::Timeout)
    }

    fn check_error_register(&mut self) -> Result<(), Error<E>> {
        let err = self.read(Register::ErrorReg).map_err(Error::Spi)?;

        if err & PROTOCOL_ERR != 0 {
            Err(Error::Protocol)
        } else if err & PARITY_ERR != 0 {
            Err(Error::Parity)
        } else if err & CRC_ERR != 0 {
            Err(Error::Crc)
        } else if err & COLL_ERR != 0 {
            Err(Error::Collision)
        } else if err & BUFFER_OVFL != 0 {
            Err(Error::BufferOverflow)
        } else if err & TEMP_ERR != 0 {
            Err(Error::Overheating)
        } else if err & WR_ERR != 0 {
            Err(Error::Wr)
        } else {
            Ok(())
        }
    }

    // Transmit + Receive
    fn transceive<const RX: usize>(
        &mut self,
        // the data to be sent
        tx_buffer: &[u8],
        // number of bits in the last byte that will be transmitted
        tx_last_bits: u8,
        // bit position for the first received bit to be stored in the FIFO buffer
        rx_align_bits: u8,
    ) -> Result<FifoData<RX>, Error<E>>
    where
        [u8; RX]: Sized,
    {
        // stop any ongoing command
        self.command(Command::Idle).map_err(Error::Spi)?;

        // clear all interrupt flags
        self.write(Register::ComIrqReg, 0x7f).map_err(Error::Spi)?;

        // flush FIFO buffer
        self.fifo_flush().map_err(Error::Spi)?;

        // write data to transmit to the FIFO buffer
        self.write_many(Register::FIFODataReg, tx_buffer)?;

        // signal command
        self.command(Command::Transceive).map_err(Error::Spi)?;

        // configure short frame and start transmission
        self.write(
            Register::BitFramingReg,
            (1 << 7) | ((rx_align_bits & 0b0111) << 4) | (tx_last_bits & 0b0111),
        )
        .map_err(Error::Spi)?;

        // TODO timeout when connection to the MFRC522 is lost (?)
        // wait for transmission + reception to complete
        let mut timeout = 0;
        loop {
            let irq = self.read(Register::ComIrqReg).map_err(Error::Spi)?;

            if irq & (RX_IRQ | ERR_IRQ | IDLE_IRQ) != 0 {
                break;
            } else if irq & TIMER_IRQ != 0 {
                return Err(Error::Timeout);
            } else if timeout > 10 {
                return Err(Error::LostCommunication);
            }
                        if irq == 0 {
                timeout = timeout + 1;
            }

        }

        self.check_error_register()?;
        self.fifo_data()
    }

    /// Get the data from the internal FIFO buffer
    fn fifo_data<const RX: usize>(&mut self) -> Result<FifoData<RX>, Error<E>> {
        let mut buffer = [0u8; RX];
        let mut valid_bytes = 0;
        let mut valid_bits = 0;

        if RX > 0 {
            valid_bytes = self.read(Register::FIFOLevelReg).map_err(Error::Spi)? as usize;
            if valid_bytes > RX {
                return Err(Error::NoRoom);
            }
            if valid_bytes > 0 {
                self.read_many(Register::FIFODataReg, &mut buffer[0..valid_bytes])?;
                valid_bits = (self.read(Register::ControlReg).map_err(Error::Spi)? & 0x07) as usize;
            }
        }

        Ok(FifoData {
            buffer,
            valid_bytes,
            valid_bits,
        })
    }

    /// Flush the internal FIFO buffer
    fn fifo_flush(&mut self) -> Result<(), E> {
        self.write(Register::FIFOLevelReg, FLUSH_BUFFER)
    }

    /// Request to execute the given command
    fn command(&mut self, command: Command) -> Result<(), E> {
        self.write(Register::CommandReg, command.into())
    }

    /// Perform a software reset
    fn reset(&mut self) -> Result<(), E> {
        self.command(Command::SoftReset)?;
        while self.read(Register::CommandReg)? & POWER_DOWN != 0 {}
        Ok(())
    }

    // lowest level API

    fn read(&mut self, reg: Register) -> Result<u8, E> {
        let mut buffer = [reg.read_address(), 0];

        self.with_nss_low(|mfr| {
            let buffer = mfr.spi.transfer(&mut buffer)?;

            Ok(buffer[1])
        })
    }

    fn read_many<'b>(&mut self, reg: Register, buffer: &'b mut [u8]) -> Result<&'b [u8], Error<E>> {
        let mut vec = Vec::<u8, 65>::new();
        let n = buffer.len();
        for _ in 0..n {
            vec.push(reg.read_address()).map_err(|_| Error::NoRoom)?;
        }
        vec.push(0).map_err(|_| Error::NoRoom)?;

        self.with_nss_low(move |mfr| {
            let res = mfr.spi.transfer(vec.as_mut()).map_err(Error::Spi)?;

            for (idx, slot) in res[1..].iter().enumerate() {
                if idx >= n {
                    break;
                }
                buffer[idx] = *slot;
            }

            Ok(&*buffer)
        })
    }

    fn rmw<F>(&mut self, reg: Register, f: F) -> Result<(), E>
    where
        F: FnOnce(u8) -> u8,
    {
        let byte = self.read(reg)?;
        self.write(reg, f(byte))?;
        Ok(())
    }

    fn write(&mut self, reg: Register, val: u8) -> Result<(), E> {
        self.with_nss_low(|mfr| mfr.spi.write(&[reg.write_address(), val]))
    }

    fn write_many(&mut self, reg: Register, bytes: &[u8]) -> Result<(), Error<E>> {
        self.with_nss_low(|mfr| {
            let mut vec = Vec::<u8, 65>::new();
            vec.push(reg.write_address()).map_err(|_| Error::NoRoom)?;
            vec.extend_from_slice(bytes).map_err(|_| Error::NoRoom)?;
            mfr.spi.write(vec.as_slice()).map_err(Error::Spi)?;

            Ok(())
        })
    }
}

/// Temporary trait to allow different implementations in case a NSS pin and/or
/// a delay function have been added to the Mfrc522.
/// The entire way of communicating to the chip needs to be refactored,
/// to also allow an implementation of I2C and UART communication.
/// When that is tackled, this trait will disappear.
pub trait WithNssDelay {
    fn with_nss_low<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T;
}

#[doc(hidden)]
impl<SPI, S: State> WithNssDelay for Mfrc522<SPI, DummyNSS, DummyDelay, S> {
    fn with_nss_low<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        f(self)
    }
}

#[doc(hidden)]
impl<SPI, D, S: State> WithNssDelay for Mfrc522<SPI, DummyNSS, D, S>
where
    D: FnMut(),
{
    fn with_nss_low<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let result = f(self);
        (self.delay)();

        result
    }
}

#[doc(hidden)]
impl<SPI, NSS, S: State> WithNssDelay for Mfrc522<SPI, NSS, DummyDelay, S>
where
    NSS: OutputPin,
{
    fn with_nss_low<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.nss.set_low();
        let result = f(self);
        self.nss.set_high();

        result
    }
}

#[doc(hidden)]
impl<SPI, NSS, D, S: State> WithNssDelay for Mfrc522<SPI, NSS, D, S>
where
    NSS: OutputPin,
    D: FnMut(),
{
    fn with_nss_low<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        self.nss.set_low();
        let result = f(self);
        self.nss.set_high();
        (self.delay)();

        result
    }
}

struct FifoData<const L: usize> {
    /// The contents of the FIFO buffer
    buffer: [u8; L],
    /// The number of valid bytes in the buffer
    valid_bytes: usize,
    /// The number of valid bits in the last byte
    valid_bits: usize,
}

impl<const L: usize> FifoData<L> {
    /// Copies FIFO data to destination buffer.
    /// Assumes the FIFO data is aligned properly to append directly to the current known bits.
    /// Returns the number of valid bits in the destination buffer after copy.
    pub fn copy_bits_to(&self, dst: &mut [u8], dst_valid_bits: u8) -> u8 {
        if self.valid_bytes == 0 {
            // nothing to copy
            return dst_valid_bits;
        }

        let dst_valid_bytes = dst_valid_bits / 8;
        let dst_valid_last_bits = dst_valid_bits % 8;
        let mask: u8 = 0xFF << dst_valid_last_bits;
        let mut idx = dst_valid_bytes as usize;
        dst[idx] = (self.buffer[0] & mask) | (dst[idx] & !mask);
        idx += 1;
        let len = self.valid_bytes - 1;
        if len > 0 {
            dst[idx..idx + len].copy_from_slice(&self.buffer[1..=len]);
        }
        dst_valid_bits + (len * 8) as u8 + self.valid_bits as u8
    }
}
