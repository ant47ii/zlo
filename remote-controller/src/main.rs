#![no_std]
#![no_main]

mod display;
mod tasks;

use embassy_stm32::adc::{ Adc, AdcChannel};
use embassy_stm32::i2c::I2c;
use embassy_stm32::rcc::{AHBPrescaler, APBPrescaler, MSIRange, Pll, PllDiv, PllMul, PllPreDiv, PllSource, Sysclk};
use embassy_stm32::time::Hertz;

use radio::{RADIO_POLL_INTERVAL_MS, init_tx_radio};
use embassy_time::{Timer};

use defmt_rtt as _;

use embassy_stm32::dma::InterruptHandler;
use embassy_stm32::{bind_interrupts, i2c, peripherals};
use panic_probe as _;

use embassy_executor::{self as _}; 
use embassy_executor::Spawner;
use embassy_stm32::gpio::{ Level, Output, Speed};
use embassy_stm32::spi::{ Spi};

#[allow(unused)]
fn setup_timestamp() {
	defmt::timestamp!("{=u64:us}", embassy_time::Instant::now().as_micros());
}

bind_interrupts!(struct Irqs {
	// NRF24
	GPDMA1_CHANNEL0 => InterruptHandler<embassy_stm32::peripherals::GPDMA1_CH0>;
	GPDMA1_CHANNEL1 => InterruptHandler<embassy_stm32::peripherals::GPDMA1_CH1>;

	// DISPLAY
	I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
	I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;
	GPDMA1_CHANNEL2 => InterruptHandler<embassy_stm32::peripherals::GPDMA1_CH2>;
	GPDMA1_CHANNEL3 => InterruptHandler<embassy_stm32::peripherals::GPDMA1_CH3>;

	// JOYSTIC
	GPDMA1_CHANNEL4 => InterruptHandler<embassy_stm32::peripherals::GPDMA1_CH4>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
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

	let mut display = display::OledDisplay::new(i2c);
	display.init().await;

	// ==========================================
	//               JOYSTICK
	// ==========================================
	let adc_config = embassy_stm32::adc::AdcConfig {
		averaging: Some(embassy_stm32::adc::Averaging::Samples16),
		resolution: Some(embassy_stm32::adc::Resolution::BITS8),
		..Default::default()
	};
	let adc = Adc::new_with_config(p.ADC1, adc_config);

    let platform_reader = joystick::Stm32JoystickAdc::new(
        p.PA2.degrade_adc(),
        p.PA3.degrade_adc(),
        adc,
        p.GPDMA1_CH4,
		Irqs
    );

 	let mut joy = joystick::Joystick::new(platform_reader);
	joy.calibrate().await;


	let mut radio_buffer = [0u8; 16];


	//let display_send = tasks::DISPLAY_WATCH.dyn_sender();
	let display_recv = tasks::DISPLAY_WATCH.dyn_receiver().unwrap();

	spawner.spawn(tasks::display_task(display_recv, display).unwrap());

	loop {
		let values = joy.read().await;
		let joy_x: i8 = joystick::apply_joystick_expo(values.x, 0.4);
		let joy_y: i8 = joystick::apply_joystick_expo(values.y, 0.4);
	
		let mut success = false; 

		if joy_x != 0 || joy_y != 0 {
			let radio_package = radio::RadioPackage::Coords(radio::JoystickCoords { x: joy_x, y: joy_y });
			success = radio::send(&mut tx, radio_package, &mut radio_buffer).await;
		}

		let current_state = tasks::DisplayState {
			tx_status: success,
			joy_x: joy_x,
			joy_y: joy_y,
		};
		
		tasks::DISPLAY_WATCH.sender().send(current_state);
		Timer::after_millis(RADIO_POLL_INTERVAL_MS).await;
	}
}
