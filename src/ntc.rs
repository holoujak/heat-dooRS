use defmt::trace;
use embassy_executor::task;
use embassy_stm32::Peri;
use embassy_stm32::adc::{Adc, SampleTime};
use embassy_stm32::peripherals::{ADC1, PA0};
use embassy_time::Timer;
use micromath::F32Ext;

use crate::SIGNAL_TEMPERATURE;

const ADC_MAX: f32 = 4095.0;
const R_PULL: f32 = 10_000.0; // pull-down 10k
const R_NTC_25: f32 = 10_000.0; // 10k @ 25°C
const BETA: f32 = 5800.0;
const T0: f32 = 298.15; // 25°C v K

pub fn adc_to_temperature_c(adc: u16) -> f32 {
    if adc == 0 || adc as f32 >= ADC_MAX {
        return f32::NAN;
    }

    let adc_f = adc as f32;

    // NTC to VCC, pull-down to GND
    let r_ntc = R_PULL * (ADC_MAX - adc_f) / adc_f;

    let inv_t = (1.0 / T0) + (1.0 / BETA) * (r_ntc / R_NTC_25).ln();

    (1.0 / inv_t) - 273.15
}

#[task]
pub async fn ntc(temp_pin: Peri<'static, PA0>, temp_adc: Peri<'static, ADC1>) {
    let mut adc = Adc::new(temp_adc);
    let mut pin = temp_pin;

    let mut vrefint = adc.enable_vref();
    adc.set_sample_time(SampleTime::CYCLES13_5);

    let vrefint_sample = adc.read(&mut vrefint).await;
    let convert_to_millivolts = |sample: u16| {
        // From http://www.st.com/resource/en/datasheet/CD00161566.pdf
        // 5.3.4 Embedded reference voltage
        const VREFINT_MV: u32 = 1200; // mV

        (u32::from(sample) * VREFINT_MV / u32::from(vrefint_sample)) as u16
    };

    loop {
        let measured = adc.read(&mut pin).await;
        trace!("--> {} - {} mV", measured, convert_to_millivolts(measured));

        let temp_c = adc_to_temperature_c(measured);

        if temp_c.is_normal() {
            trace!("Temperature: {}", temp_c);
            SIGNAL_TEMPERATURE.signal(temp_c);
        }

        Timer::after_millis(1000).await;
    }
}
