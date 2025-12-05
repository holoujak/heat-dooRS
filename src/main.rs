#![no_std]
#![no_main]

mod ds18b20;
mod onewire;

use cortex_m::singleton;
use defmt::{error, info};
use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::mode::Async;
use embassy_stm32::peripherals::*;
use embassy_stm32::usart::{self, RingBufferedUartRx, Uart, UartTx};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::Duration;
use embassy_time::Timer;
use onewire::OneWire;
#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct OneWireUsartIrqs {
    USART1 => usart::InterruptHandler<USART1>;
});

pub static SIGNAL_TEMPERATURE: Signal<CriticalSectionRawMutex, Option<u16>> = Signal::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    SIGNAL_TEMPERATURE.signal(None);

    // Configure USART for OneWire communication
    let mut usart_config = usart::Config::default();
    usart_config.baudrate = 115200; // Standard OneWire baudrate

    let onewire_usart = Uart::new_half_duplex(
        p.USART1,
        p.PB6,
        OneWireUsartIrqs,
        p.DMA1_CH4, // USART1_TX uses DMA1 Channel 4
        p.DMA1_CH5, // USART1_RX uses DMA1 Channel 5
        usart_config,
        // Enable readback so we can read sensor pulling data low while transmission is in progress
        usart::HalfDuplexReadback::Readback,
        //usart::HalfDuplexConfig::OpenDrainExternal,
    )
    .unwrap();

    const BUFFER_SIZE: usize = 16;
    let (tx, rx) = onewire_usart.split();
    let rx_buf: &mut [u8; BUFFER_SIZE] =
        singleton!(TX_BUF: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE]).unwrap();
    let rx = rx.into_ring_buffered(rx_buf);
    let onewire = OneWire::new(tx, rx);

    let led_pin = Output::new(p.PC13, Level::High, Speed::Low);

    spawner.spawn(led_task(led_pin)).unwrap();
    spawner.spawn(ds18b20_task(onewire)).unwrap();
}

#[embassy_executor::task]
async fn led_task(mut led_pin: Output<'static>) {
    info!("Starting LED task");
    loop {
        led_pin.toggle();
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[embassy_executor::task]
async fn ds18b20_task(onewire: OneWire<UartTx<'static, Async>, RingBufferedUartRx<'static>>) {
    info!("Starting DS18B20 task");
    let mut sensor: ds18b20::Ds18b20<UartTx<'_, Async>, RingBufferedUartRx<'_>> =
        ds18b20::Ds18b20::new(onewire);
    loop {
        SIGNAL_TEMPERATURE.signal(match sensor.raw_temperature().await {
            Ok(raw_temperature) => {
                info!("Temperature: {}°C", raw_temperature as f32 / 16.);
                Some(raw_temperature)
            }
            Err(_) => {
                error!("Failed to read temperature");
                None
            }
        });

        Timer::after(Duration::from_secs(1)).await;
    }
}
