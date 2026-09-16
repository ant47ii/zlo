use embedded_graphics::{
	pixelcolor::BinaryColor,
	prelude::*,
};
use ssd1306::{Ssd1306, Ssd1306Async, mode::{BufferedGraphicsMode, BufferedGraphicsModeAsync}, prelude::*};
//use u8g2_fonts::{FontRenderer, fonts, types::{FontColor, HorizontalAlignment, VerticalPosition}};

pub struct OledDisplay<DI> {
	display: Ssd1306Async<DI, DisplaySize128x64, BufferedGraphicsModeAsync<DisplaySize128x64>>,
//	font: FontRenderer
}

impl<DI> OledDisplay<DI> 
where 
	DI: WriteOnlyDataCommand
{

	pub fn new(interface: DI) -> Self {
		let display = Ssd1306Async::new(interface, DisplaySize128x64, DisplayRotation::Rotate0).into_buffered_graphics_mode();
		
		
		//let font = FontRenderer::new::<fonts::u8g2_font_6x13_t_cyrillic>();
		Self { display/* , font */}
	}

	pub async fn init(&mut self)  {

		/*match self.display.init().await {
			Ok(_) => {},
			Err(_) => {  }
		}*/


		//self.display.clear(BinaryColor::Off);
		//self.display.flush();
	}

	pub fn draw(&mut self /* все метрики */) {

	}

	/*fn print(&mut self, text: &str) -> Result<(), OledError> {
        match self.font.render_aligned(
            text,
            Point::new(5, 5),
            VerticalPosition::Top,
            HorizontalAlignment::Left,
            FontColor::Transparent(BinaryColor::On),
            &mut self.display,
        ) {
            Ok(Some(_)) => Ok(()),
            _ => Err(OledError::Error),
        }
	}*/

}