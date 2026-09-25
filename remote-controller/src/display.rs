use embedded_graphics::{
	geometry::Point,
	mono_font::{iso_8859_1::FONT_6X10, MonoTextStyleBuilder},
	pixelcolor::BinaryColor,
	text::Text,
	Drawable
};
use ssd1306::{
	I2CDisplayInterface, Ssd1306Async, mode::BufferedGraphicsModeAsync, prelude::*, rotation::DisplayRotation, size::DisplaySize128x64,
};

pub struct OledDisplay<I2C> {
	display: Ssd1306Async<I2CInterface<I2C>, DisplaySize128x64, BufferedGraphicsModeAsync<DisplaySize128x64>>
}

impl<I2C> OledDisplay<I2C>
where
	I2C: embedded_hal_async::i2c::I2c + 'static
{
	pub fn new(i2c_bus: I2C) -> Self {
		let interface = I2CDisplayInterface::new(i2c_bus);
		let display = Ssd1306Async::new(interface,DisplaySize128x64,DisplayRotation::Rotate0,).into_buffered_graphics_mode();

		Self { display }
	}

	pub async fn init(&mut self) {
		if self.display.init().await.is_ok() {
			self.clear_buffer();
			self.update().await;
		}
	}

	pub async fn update(&mut self) {
		let _ = self.display.flush().await;
	}
	
	pub fn clear_buffer(&mut self) {
		self.display.clear_buffer();
	}

	pub fn write_line(&mut self, text: &str, position: Point) {
		let text_style = MonoTextStyleBuilder::new()
			.font(&FONT_6X10)
			.text_color(BinaryColor::On)
			.build();

		let _ = Text::new(text, position, text_style).draw(&mut self.display);
	}
}