#![no_std]
#![no_main]

mod ntc;

use crate::ntc::ntc;
use defmt::info;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::peripherals::*;
use embassy_stm32::{adc, bind_interrupts};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::Duration;
use embassy_time::Timer;

#[cfg(not(feature = "defmt"))]
use panic_halt as _;

#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    ADC1_2 => adc::InterruptHandler<ADC1>;
});

pub static SIGNAL_TEMPERATURE: Signal<CriticalSectionRawMutex, f32> = Signal::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    SIGNAL_TEMPERATURE.signal(0.0);

    let led_pin = Output::new(p.PC13, Level::High, Speed::Low);

    spawner.spawn(led_task(led_pin)).unwrap();
    spawner.spawn(ntc(p.PA0, p.ADC1)).unwrap();
}

#[embassy_executor::task]
async fn led_task(mut led_pin: Output<'static>) {
    info!("Starting LED task");
    loop {
        led_pin.toggle();
        Timer::after(Duration::from_secs(1)).await;
    }
}
