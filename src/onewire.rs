use defmt::*;
use embassy_stm32::mode::Async;
use embassy_stm32::usart::{
    BufferedUartRx, BufferedUartTx, ConfigError, RingBufferedUartRx, UartTx,
};
use {defmt_rtt as _, panic_probe as _};

/// Simplified OneWire bus driver
pub struct OneWire<TX, RX>
where
    TX: embedded_io_async::Write + SetBaudrate,
    RX: embedded_io_async::Read + SetBaudrate,
{
    tx: TX,
    rx: RX,
}

impl<TX, RX> OneWire<TX, RX>
where
    TX: embedded_io_async::Write + SetBaudrate,
    RX: embedded_io_async::Read + SetBaudrate,
{
    // bitrate with one bit taking ~104 us
    const RESET_BAUDRATE: u32 = 9600;
    // bitrate with one bit taking ~8.7 us
    const BAUDRATE: u32 = 115200;

    // startbit + 8 low bits = 9 * 1/115200 = 78 us low pulse
    const LOGIC_1_CHAR: u8 = 0xFF;
    // startbit only = 1/115200 = 8.7 us low pulse
    const LOGIC_0_CHAR: u8 = 0x00;

    // Address all devices on the bus
    pub const COMMAND_SKIP_ROM: u8 = 0xCC;

    pub fn new(tx: TX, rx: RX) -> Self {
        Self { tx, rx }
    }

    fn set_baudrate(&mut self, baudrate: u32) -> Result<(), ConfigError> {
        self.tx.set_baudrate(baudrate)?;
        self.rx.set_baudrate(baudrate)
    }

    /// Reset the bus by at least 480 us low pulse.
    pub async fn reset(&mut self) {
        info!("OneWire reset starting...");
        // Switch to 9600 baudrate, so one bit takes ~104 us
        self.set_baudrate(Self::RESET_BAUDRATE)
            .expect("set_baudrate failed");
        info!("Baudrate set to 9600");

        // Clear any pending data in RX buffer
        let mut dummy = [0u8; 32];
        loop {
            match embassy_time::with_timeout(
                embassy_time::Duration::from_millis(1),
                self.rx.read(&mut dummy),
            )
            .await
            {
                Ok(Ok(n)) if n > 0 => {
                    info!("Cleared {} bytes from RX buffer", n);
                    continue;
                }
                _ => break,
            }
        }

        // Low USART start bit + 4x low bits = 5 * 104 us = 520 us low pulse
        self.tx.write_all(&[0xF0]).await.expect("write failed");
        info!("Reset pulse sent");

        // Wait for the byte to be transmitted (1 start + 8 data + 1 stop = ~1040us at 9600 baud)
        embassy_time::Timer::after(embassy_time::Duration::from_micros(1100)).await;

        // Read the value on the bus - should get echo + sensor response
        let mut buffer = [0; 1];
        match embassy_time::with_timeout(
            embassy_time::Duration::from_millis(10),
            self.rx.read(&mut buffer),
        )
        .await
        {
            Ok(Ok(n)) if n > 0 => {
                info!("OneWire reset response: {:#x} ({} bytes)", buffer[0], n);
            }
            Ok(Ok(_)) => {
                warn!("Read returned 0 bytes");
                buffer[0] = 0xFF;
            }
            Ok(Err(_)) => {
                warn!("Read error during reset");
                buffer[0] = 0xFF;
            }
            Err(_) => {
                warn!("Timeout waiting for reset response");
                buffer[0] = 0xFF;
            }
        }

        // Switch back to 115200 baudrate, so one bit takes ~8.7 us
        self.set_baudrate(Self::BAUDRATE)
            .expect("set_baudrate failed");
        info!("Baudrate set back to 115200");

        // read and expect sensor pulled some high bits to low (device present)
        if buffer[0] & 0xF != 0 || buffer[0] & 0xF0 == 0xF0 {
            warn!("No device present (response: {:#x})", buffer[0]);
        } else {
            info!("Device present!");
        }
    }

    /// Send byte and read response on the bus.
    pub async fn write_read_byte(&mut self, byte: u8) -> u8 {
        // One byte is sent as 8 UART characters
        let mut tx = [0; 8];
        for (pos, char) in tx.iter_mut().enumerate() {
            *char = if (byte >> pos) & 0x1 == 0x1 {
                Self::LOGIC_1_CHAR
            } else {
                Self::LOGIC_0_CHAR
            };
        }
        self.tx.write_all(&tx).await.expect("write failed");

        // Readback the value on the bus, sensors can pull logic 1 to 0
        let mut rx = [0; 8];
        self.rx.read_exact(&mut rx).await.expect("read failed");
        let mut bus_byte = 0;
        for (pos, char) in rx.iter().enumerate() {
            // if its 0xFF, sensor didnt pull the bus to low level
            if *char == 0xFF {
                bus_byte |= 1 << pos;
            }
        }

        bus_byte
    }

    /// Read a byte from the bus.
    pub async fn read_byte(&mut self) -> u8 {
        self.write_read_byte(0xFF).await
    }
}

pub trait SetBaudrate {
    fn set_baudrate(&mut self, baudrate: u32) -> Result<(), ConfigError>;
}

impl SetBaudrate for BufferedUartTx<'_> {
    fn set_baudrate(&mut self, baudrate: u32) -> Result<(), ConfigError> {
        BufferedUartTx::set_baudrate(self, baudrate)
    }
}
impl SetBaudrate for BufferedUartRx<'_> {
    fn set_baudrate(&mut self, baudrate: u32) -> Result<(), ConfigError> {
        BufferedUartRx::set_baudrate(self, baudrate)
    }
}
impl SetBaudrate for RingBufferedUartRx<'_> {
    fn set_baudrate(&mut self, baudrate: u32) -> Result<(), ConfigError> {
        RingBufferedUartRx::set_baudrate(self, baudrate)
    }
}
impl SetBaudrate for UartTx<'_, Async> {
    fn set_baudrate(&mut self, baudrate: u32) -> Result<(), ConfigError> {
        UartTx::set_baudrate(self, baudrate)
    }
}
