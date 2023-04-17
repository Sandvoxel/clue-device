/// Errors
#[derive(Debug)]
pub enum Error<E> {
    /// Wrong Block Character Check (BCC)
    Bcc,
    /// FIFO buffer overflow
    BufferOverflow,
    /// Collision
    Collision,
    /// Wrong CRC
    Crc,
    /// Incomplete RX frame
    IncompleteFrame,
    /// Provided buffer not large enough
    NoRoom,
    /// Internal temperature sensor detects overheating
    Overheating,
    /// Parity check failed
    Parity,
    /// Error during MFAuthent operation
    Protocol,
    /// SPI bus error
    Spi(E),
    /// Timeout
    Timeout,
    /// Write error: FIFO buffer was written at invalid time
    Wr,
    /// Not acknowledge
    Nak,
    /// Proprietary frames, commands or protocols used
    Proprietary,
    ///Lost communication with mfq
    LostCommunication
}

#[cfg(feature = "std")]
impl<E> std::fmt::Display for Error<E> {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<E> std::error::Error for Error<E>
where
    E: std::fmt::Debug,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}
