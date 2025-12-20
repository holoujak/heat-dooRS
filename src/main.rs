#![no_std]
#![no_main]

mod motor_control;
mod ntc;

use crate::motor_control::{MotorControl, MotorStatus, motor_control};
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
pub static SIGNAL_MOTOR_STATUS: Signal<CriticalSectionRawMutex, MotorStatus> = Signal::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    SIGNAL_TEMPERATURE.signal(0.0);

    let led_pin = Output::new(p.PC13, Level::High, Speed::Low);
    let motor_en_pin = Output::new(p.PA1, Level::Low, Speed::Low);
    let motor_dir_pin = Output::new(p.PA2, Level::Low, Speed::Low);

    let motor = MotorControl::new(motor_dir_pin, motor_en_pin);
    spawner.spawn(led_task(led_pin)).unwrap();
    spawner.spawn(ntc(p.PA0, p.ADC1)).unwrap();
    spawner.spawn(motor_control(motor)).unwrap();
}

#[embassy_executor::task]
async fn led_task(mut led_pin: Output<'static>) {
    info!("Starting LED task");
    let mut current_status = MotorStatus::Off;
    loop {
        if let Some(status) = SIGNAL_MOTOR_STATUS.try_take() {
            current_status = status;
        }
        match current_status {
            MotorStatus::Closing => {
                led_pin.set_low();
            }
            MotorStatus::Opening => {
                led_pin.toggle();
            }
            MotorStatus::Off => {
                led_pin.set_high();
            }
        }
        Timer::after(Duration::from_millis(100)).await;
    }
}
