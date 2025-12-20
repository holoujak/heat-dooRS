use defmt::info;
use embassy_executor::task;
use embassy_stm32::gpio::Output;
use embassy_time::{Instant, Timer};
use micromath::F32Ext;

use crate::SIGNAL_MOTOR_STATUS;
use crate::SIGNAL_TEMPERATURE;
const MAX_TEMPERATURE: f32 = 55.0;
const MAX_MOVE_TIME: u64 = 13;
const STEP_MOVE_TIME: u64 = 1;
const TEMP_HYSTERESIS: f32 = 5.0;
const WAIT_TIME_S: u64 = 120;

#[derive(PartialEq, Clone, Copy)]
pub enum MotorStatus {
    Off,
    Opening,
    Closing,
}

pub enum HeatingStatus {
    Off,
    Heating,
    Cooling,
}

pub struct MotorControl {
    direction_pin: Output<'static>,
    enable_pin: Output<'static>,
    status: MotorStatus,
    move_start: Option<Instant>,
    total_movement_time: u64, // Movement time in any direction
    heating_status: HeatingStatus,
    last_move_status: MotorStatus,
    last_temp: f32,
}

impl MotorControl {
    pub fn new(direction_pin: Output<'static>, enable_pin: Output<'static>) -> Self {
        Self {
            direction_pin,
            enable_pin,
            status: MotorStatus::Off,
            move_start: None,
            total_movement_time: 0,
            heating_status: HeatingStatus::Off,
            last_move_status: MotorStatus::Off,
            last_temp: 0.0,
        }
    }

    pub async fn move_motor(&mut self, direction: MotorStatus, duration: u64) -> bool {
        if !self.can_move(direction) {
            info!("Max movement time reached, cannot move motor further");
            self.stop();
            return false;
        }

        match direction {
            MotorStatus::Opening => {
                info!("Opening motor for {}s", duration);
                self.open();
            }
            MotorStatus::Closing => {
                info!("Closing motor for {}s", duration);
                self.close();
            }
            _ => return false,
        }

        Timer::after_secs(duration).await;
        self.stop();
        true
    }

    pub async fn step_move(&mut self, direction: MotorStatus, temp: f32) -> bool {
        let action = match direction {
            MotorStatus::Opening => "Opening",
            MotorStatus::Closing => "Closing",
            _ => return false,
        };

        info!(
            "{} motor for one step, CUR: {}, LAST: {}",
            action, temp, self.last_temp
        );
        let success = self.move_motor(direction, STEP_MOVE_TIME).await;
        if success {
            self.last_temp = temp;
        }
        success
    }

    pub fn stop(&mut self) {
        if self.status != MotorStatus::Off {
            self.last_move_status = self.status;

            if let Some(elapsed) = self.elapsed_s() {
                self.total_movement_time += elapsed;
            }
        }

        self.move_start = None;
        self.enable_pin.set_low();
        self.direction_pin.set_low();
        self.status = MotorStatus::Off;
        SIGNAL_MOTOR_STATUS.signal(MotorStatus::Off);
    }

    pub fn close(&mut self) {
        if self.last_move_status != MotorStatus::Closing {
            self.total_movement_time = 0;
        }

        self.move_start = Some(Instant::now());

        self.enable_pin.set_high();
        self.direction_pin.set_low();
        self.status = MotorStatus::Closing;
        SIGNAL_MOTOR_STATUS.signal(MotorStatus::Closing);
    }

    pub fn open(&mut self) {
        if self.last_move_status != MotorStatus::Opening {
            self.total_movement_time = 0;
        }

        self.move_start = Some(Instant::now());

        self.enable_pin.set_high();
        self.direction_pin.set_high();
        self.status = MotorStatus::Opening;
        SIGNAL_MOTOR_STATUS.signal(MotorStatus::Opening);
    }

    pub fn can_move(&self, direction: MotorStatus) -> bool {
        let total_time = if direction != self.last_move_status {
            0
        } else {
            self.total_movement_time
        };

        total_time < MAX_MOVE_TIME
    }

    fn elapsed_s(&self) -> Option<u64> {
        self.move_start.map(|t| t.elapsed().as_secs())
    }
}

#[task]
pub async fn motor_control(mut motor_control: MotorControl) {
    loop {
        if let Some(temp) = SIGNAL_TEMPERATURE.try_take() {
            let temp = (temp * 10.0).round() / 10.0;
            info!("Temperature: {}", temp);
            match motor_control.heating_status {
                HeatingStatus::Off => {
                    // Initial setup - fully open the motor
                    info!("Opening at beginning");
                    if motor_control
                        .move_motor(MotorStatus::Opening, MAX_MOVE_TIME)
                        .await
                    {
                        info!("Motor fully open at beginning");
                        motor_control.heating_status = HeatingStatus::Heating;
                    }
                }

                HeatingStatus::Cooling => {
                    if temp < MAX_TEMPERATURE - TEMP_HYSTERESIS {
                        info!("Motor cool enough, starting heating");
                        motor_control.heating_status = HeatingStatus::Heating;
                        if motor_control
                            .move_motor(MotorStatus::Opening, MAX_MOVE_TIME)
                            .await
                        {
                            info!("Motor fully open after cool down");
                        }
                    } else {
                        info!("Cooling ...");
                    }

                    motor_control.last_temp = temp
                }

                HeatingStatus::Heating => {
                    if temp > MAX_TEMPERATURE {
                        // Overheating - fully close motor
                        info!("Closing motor to overheating");
                        if motor_control
                            .move_motor(MotorStatus::Closing, MAX_MOVE_TIME)
                            .await
                        {
                            info!("Motor fully close due to overheating");
                        }
                        motor_control.heating_status = HeatingStatus::Cooling;
                    } else if temp < MAX_TEMPERATURE - TEMP_HYSTERESIS {
                        info!("Too low temperature during heating, keep open");
                    } else {
                        // Fine-tune motor position based on temperature changes
                        if temp > motor_control.last_temp {
                            motor_control.step_move(MotorStatus::Closing, temp).await;
                        } else if temp < motor_control.last_temp {
                            motor_control.step_move(MotorStatus::Opening, temp).await;
                        }
                    }
                }
            }
        } else {
            // No new temperature, keep motor closed
            motor_control.stop();
        }

        Timer::after_secs(WAIT_TIME_S).await;
    }
}
