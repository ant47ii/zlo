#![no_std]
#![no_main]

mod joystick;
mod joystick_stm32;
mod display;

use embassy_stm32::adc::{ Adc, AdcChannel};
use embassy_stm32::i2c::I2c;
use embassy_stm32::rcc::{AHBPrescaler, APBPrescaler, MSIRange, Pll, PllDiv, PllMul, PllPreDiv, PllSource, Sysclk};
use embassy_stm32::time::Hertz;
use embedded_graphics::Drawable;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::mono_font::iso_8859_1::FONT_6X10;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::text::Text;
use heapless::String;
use radio::{RADIO_POLL_INTERVAL_MS, init_tx_radio};
use ssd1306::mode::{DisplayConfigAsync};
use ssd1306::rotation::DisplayRotation;
use ssd1306::size::DisplaySize128x64;
use ssd1306::{I2CDisplayInterface, Ssd1306Async};

use embassy_time::{Timer};

use defmt::{error, info};
use defmt_rtt as _;

use embassy_stm32::dma::InterruptHandler;
use embassy_stm32::{bind_interrupts, i2c, peripherals};
use panic_halt as _;

use embassy_executor::{self as _}; 
use embassy_executor::Spawner;
use embassy_stm32::gpio::{ Level, Output, Speed};
use embassy_stm32::spi::{ Spi};

use core::fmt::Write;

#[allow(unused)]
fn setup_timestamp() {
	defmt::timestamp!("{=u64:us}", embassy_time::Instant::now().as_micros());
}

bind_interrupts!(struct Irqs {
	// DISPLAY
	I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
	I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;
	GPDMA1_CHANNEL2 => InterruptHandler<embassy_stm32::peripherals::GPDMA1_CH2>;
	GPDMA1_CHANNEL3 => InterruptHandler<embassy_stm32::peripherals::GPDMA1_CH3>;

	// NRF24
	GPDMA1_CHANNEL0 => InterruptHandler<embassy_stm32::peripherals::GPDMA1_CH0>;
	GPDMA1_CHANNEL1 => InterruptHandler<embassy_stm32::peripherals::GPDMA1_CH1>;

	// JOYSTIC
	GPDMA1_CHANNEL4 => InterruptHandler<embassy_stm32::peripherals::GPDMA1_CH4>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
	// ==========================================
	//                 SYS
	// ==========================================
	/*let mut config = embassy_stm32::Config::default();

	config.rcc.msis = Some(MSIRange::RANGE_48MHZ);
	config.rcc.sys = Sysclk::MSIS;
	config.rcc.ahb_pre = AHBPrescaler::DIV1;
	config.rcc.apb1_pre = APBPrescaler::DIV1;
	config.rcc.apb2_pre = APBPrescaler::DIV1;
	config.rcc.apb3_pre = APBPrescaler::DIV1;

	let p = embassy_stm32::init(config);*/
	let mut config = embassy_stm32::Config::default();

	config.rcc.msis = Some(MSIRange::RANGE_4MHZ);
	config.rcc.sys = Sysclk::PLL1_R;
	
	config.rcc.pll1 = Some(Pll {
		source: PllSource::MSIS,     // Источник: MSIS (4 МГц)
		prediv: PllPreDiv::DIV1,     // Предделитель: /1 (вход PLL = 4 МГц)
		mul: PllMul::MUL40,         // Умножитель: *50 (VCO = 200 МГц)
		divp: None,
		divq: None,
		divr: Some(PllDiv::DIV2),   // Выход R: /2 (200 МГц / 2 = 100 МГц)
	});

	config.rcc.ahb_pre = AHBPrescaler::DIV1;
	config.rcc.apb1_pre = APBPrescaler::DIV1;
	config.rcc.apb2_pre = APBPrescaler::DIV1;
	config.rcc.apb3_pre = APBPrescaler::DIV1;


	let p = embassy_stm32::init(config);
	
	// A 0  кнопка
	// A 4 5 6 7 заняты под флэш
	
	// A 11 12 USB
	// А 13 14 SWD

	// C 13 led
	// B 2  резистор какой-то (boot)
	

	// ==========================================
	//                 NRF24
	// ==========================================
	/*
	+-----------+
	| GND | VCC |  
	|  CE | CSN |  <- (PA8),  (PB12)
	| SCK | MOSI|  <- (PB13), (PB15)
	| MISO|     |  <- (PB14),
	+-----------+
	*/
	let ce = Output::new(p.PA8, Level::Low, Speed::VeryHigh);
	let cs = Output::new(p.PB12, Level::High, Speed::VeryHigh);
	let mut spi_config = embassy_stm32::spi::Config::default();
	spi_config.frequency = Hertz::mhz(2);
	
	let spi_bus = Spi::new(
		p.SPI2, 
		p.PB13, // SCLK
		p.PB15, // MOSI
		p.PB14, // MISO
		p.GPDMA1_CH0, // DMA Tx
		p.GPDMA1_CH1, // DMA Rx
		Irqs,
		spi_config
	);

	let spi_device = embedded_hal_bus::spi::ExclusiveDevice::new(
		spi_bus, 
		cs, 
		embassy_time::Delay
	).unwrap();

	let mut tx = init_tx_radio(ce, spi_device).await;

	// ==========================================
	//                 DISPLAY
	// ==========================================
	let i2c = I2c::new(
		p.I2C1,
		p.PB8,
		p.PB9,
		p.GPDMA1_CH2,
		p.GPDMA1_CH3,
		Irqs,
		embassy_stm32::i2c::Config::default(),
	);

	let interface = I2CDisplayInterface::new(i2c);
	let mut display = Ssd1306Async::new(interface, DisplaySize128x64, DisplayRotation::Rotate0).into_buffered_graphics_mode();
	display.init().await.unwrap();
	display.clear(BinaryColor::Off).unwrap();

	let text_style = MonoTextStyleBuilder::new()
		.font(&FONT_6X10)
		.text_color(BinaryColor::On)
		.build();

	// ==========================================
	//               JOYSTICK
	// ==========================================
	let adc_config = embassy_stm32::adc::AdcConfig {
		averaging: Some(embassy_stm32::adc::Averaging::Samples16),
		resolution: Some(embassy_stm32::adc::Resolution::BITS8),
		..Default::default()
	};
	let adc = Adc::new_with_config(p.ADC1, adc_config);
	
    let platform_reader = joystick_stm32::Stm32JoystickAdc::new(
        p.PA2.degrade_adc(),
        p.PA3.degrade_adc(),
        adc,
        p.GPDMA1_CH4,
    );

 	let mut joystick = joystick::Joystick::new(platform_reader);
	joystick.calibrate().await;




	let mut text_buffer: String<32> = String::new();
	let mut radio_buffer = [0u8; 16];

	loop {

		let values = joystick.read().await;
		let joy_x: i8 = joystick::apply_joystick_expo(values.x, 0.4);
		let joy_y: i8 = joystick::apply_joystick_expo(values.y, 0.4);

		display.clear(BinaryColor::Off).unwrap();
		text_buffer.clear();
		write!(text_buffer, "{} : {}", joy_x, joy_y).unwrap();

		Text::new(&text_buffer, Point::new(10, 20), text_style)
			.draw(&mut display)
			.unwrap();
		
		if joy_x != 0 || joy_y != 0 {
			let radio_package = radio::RadioPackage::Coords(radio::JoystickCoords { x: joy_x, y: joy_y });
			let success = radio::send(&mut tx, radio_package, &mut radio_buffer).await;
			
			if success {
				Text::new("OK", Point::new(10, 40), text_style)
					.draw(&mut display)
					.unwrap();
			} else {
				Text::new("ERROR", Point::new(10, 40), text_style)
					.draw(&mut display)
					.unwrap();
			}
		}

		display.flush().await.unwrap();
		Timer::after_millis(RADIO_POLL_INTERVAL_MS).await;
	}
}
