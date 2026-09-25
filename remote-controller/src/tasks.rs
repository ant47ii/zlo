use defmt::info;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, watch::{DynReceiver, Watch}};
use embassy_time::{Duration, Timer, with_timeout};
use embedded_graphics::{geometry::Point, mono_font::{MonoTextStyleBuilder, iso_8859_1::FONT_6X10}, pixelcolor::BinaryColor, text::Text};
use embedded_graphics::Drawable;

use heapless::String;
use ssd1306::{prelude::*, Ssd1306Async, size::DisplaySize128x64, mode::BufferedGraphicsModeAsync};
use embassy_stm32::i2c::{I2c, Master};
use embassy_stm32::mode::Async;

use core::fmt::Write;

pub static DISPLAY_WATCH: Watch<CriticalSectionRawMutex, DisplayState, 1> = Watch::new();

#[derive(Clone, Copy)]
pub struct DisplayState {
    pub tx_status: bool,
    pub joy_x: i8,
    pub joy_y: i8,
}

type JoystickDisplay = Ssd1306Async<
	I2CInterface<I2c<'static, Async, Master>>, 
	DisplaySize128x64, 
	BufferedGraphicsModeAsync<DisplaySize128x64>
>;

#[embassy_executor::task]
pub async fn display_task(
	mut display_receiver: DynReceiver<'static, DisplayState>,
	mut display: JoystickDisplay
) {
	info!("Задание дисплея запущено");
	
	let text_style = MonoTextStyleBuilder::new()
		.font(&FONT_6X10)
		.text_color(BinaryColor::On)
		.build();

	let mut text_buffer: String<32> = String::new();

	loop {
		match with_timeout(Duration::from_millis(100), display_receiver.changed()).await { // TODO: const
			Ok(state) => {
				display.clear_buffer();
				//display.clear(BinaryColor::Off).unwrap();
				text_buffer.clear();
				write!(text_buffer, "{} : {}", state.joy_x, state.joy_y).unwrap();
				Text::new(&text_buffer, Point::new(10, 20), text_style)
					.draw(&mut display)
					.unwrap();

				if state.tx_status {
					Text::new("OK", Point::new(10, 40), text_style)
						.draw(&mut display)
						.unwrap();
				} else {
					Text::new("ERROR", Point::new(10, 40), text_style)
						.draw(&mut display)
						.unwrap();
				}

				display.flush().await.unwrap();
			}
			Err(_) => { }
		}

		Timer::after_millis(100).await; // TODO: const
	}
}