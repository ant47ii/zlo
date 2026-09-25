use defmt::info;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, watch::{DynReceiver, Watch}};
use embassy_time::{Duration, Timer, with_timeout};
use embedded_graphics::{geometry::Point};

use heapless::String;
use embassy_stm32::i2c::{I2c, Master};
use embassy_stm32::mode::Async;

use core::fmt::Write;

use crate::display::OledDisplay;

pub static DISPLAY_WATCH: Watch<CriticalSectionRawMutex, DisplayState, 1> = Watch::new();

#[derive(Clone, Copy)]
pub struct DisplayState {
    pub tx_status: bool,
    pub joy_x: i8,
    pub joy_y: i8,
}

#[embassy_executor::task]
pub async fn display_task(
	mut display_receiver: DynReceiver<'static, DisplayState>,
	mut display: OledDisplay<I2c<'static, Async, Master>>
) {
	info!("Задание дисплея запущено");
	
	let mut text_buffer: String<32> = String::new();

	loop {
		match with_timeout(Duration::from_millis(100), display_receiver.changed()).await { // TODO: const
			Ok(state) => {
				display.clear_buffer();
				text_buffer.clear();

				write!(text_buffer, "{} : {}", state.joy_x, state.joy_y).unwrap();
				display.write_line(&text_buffer, Point::new(10, 20));

				if state.tx_status {
					display.write_line("OK", Point::new(10, 40));
				} else {
					display.write_line("ERROR", Point::new(10, 40));
				}

				display.update().await;
			}
			Err(_) => { }
		}

		Timer::after_millis(100).await; // TODO: const
	}
}