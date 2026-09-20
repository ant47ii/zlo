#![no_std]
#![no_main]

mod tasks;
mod drive;

use crate::drive::{RoverDrive};
use crate::tasks::{motor_task, radio_task, hardware_task};
use ::radio::{ init_rx_radio};

use embassy_stm32::timer::simple_pwm::{PwmPin, SimplePwm};

use embassy_stm32::rcc::{AHBPrescaler, APBPrescaler, Pll, PllMul, PllPreDiv, PllRDiv, PllSource, Sysclk};
use embassy_stm32::time::{Hertz};
use defmt_rtt as _;
use embassy_stm32::{bind_interrupts, peripherals};
use panic_probe as _;
use embassy_executor::{self as _};
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, OutputType, Speed};
use embassy_stm32::spi::Spi;

#[allow(unused)]
fn setup_timestamp() {
    defmt::timestamp!("{=u64:us}", embassy_time::Instant::now().as_micros());
}

bind_interrupts!(struct Irqs {
	// NRF24
	DMA1_CHANNEL1 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH1>;
	DMA1_CHANNEL2 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH2>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
	// ==========================================
	//                 SYS
	// ==========================================
	let mut config = embassy_stm32::Config::default();
	config.rcc.pll = Some(Pll {
		source: PllSource::HSI,
		prediv: PllPreDiv::DIV4,
		mul: PllMul::MUL50,
		divr: Some(PllRDiv::DIV2),
		divp: None,
		divq: None,
	});
	config.rcc.sys = Sysclk::PLL1_R;
	config.rcc.ahb_pre = AHBPrescaler::DIV1;
	config.rcc.apb1_pre = APBPrescaler::DIV1;
	config.rcc.apb2_pre = APBPrescaler::DIV1;
	let p = embassy_stm32::init(config);

	// ==========================================
	//               DRV8833
	// ==========================================

	let mut stby = Output::new(p.PA12, Level::Low, Speed::Low);

	let bin2_pin = PwmPin::new(p.PA8, OutputType::PushPull);
	let bin1_pin = PwmPin::new(p.PA9, OutputType::PushPull);
	let ain1_pin = PwmPin::new(p.PA10, OutputType::PushPull);
	let ain2_pin = PwmPin::new(p.PA11, OutputType::PushPull);

	let pwm_tim1 = SimplePwm::new(
		p.TIM1,
		Some(bin2_pin),
		Some(bin1_pin),
		Some(ain1_pin),
		Some(ain2_pin),
		Hertz::hz(20_000),
		Default::default(),
	);

	let mut parts = pwm_tim1.split(); 

	parts.ch1.enable();
	parts.ch2.enable();
	parts.ch3.enable();
	parts.ch4.enable();

	let drive = RoverDrive::new(
		parts.ch1, 
		parts.ch2, 
		parts.ch3, 
		parts.ch4
	);

	stby.set_high();

	// ==========================================
	//                 NRF24
	// ==========================================
	/*
	+-----------+
	| GND | VCC |  
	|  CE | CSN |  <- (PA3),  (PA4)
	| SCK | MOSI|  <- (PA5),  (PA7)
	| MISO|     |  <- (PA6),
	+-----------+
	*/
	let ce = Output::new(p.PA3, Level::Low, Speed::VeryHigh);
	let cs = Output::new(p.PA4, Level::High, Speed::VeryHigh);
	
	let mut spi_config = embassy_stm32::spi::Config::default();
	spi_config.frequency = Hertz::mhz(10);

	let spi_bus = Spi::new(
		p.SPI1,
		p.PA5, // SCLK
		p.PA7, // MOSI
		p.PA6, // MISO
		p.DMA1_CH1,// DMA Tx
		p.DMA1_CH2, // DMA Rx
		Irqs,
		spi_config
	);

	let spi_device = embedded_hal_bus::spi::ExclusiveDevice::new(
		spi_bus, 
		cs, 
		embassy_time::Delay
	).unwrap();

	let rx = init_rx_radio(ce, spi_device).await;

	// ==========================================
	//                 MAIN
	// ==========================================

	// Интерфейсы для координат (Watch)
	let coords_send = tasks::COORDS_WATCH.dyn_sender();
	let coords_recv = tasks::COORDS_WATCH.dyn_receiver().unwrap();

	// Интерфейсы для команд (Channel)
	let cmd_send = tasks::CMD_CHANNEL.dyn_sender();
	let cmd_recv = tasks::CMD_CHANNEL.dyn_receiver();

	spawner.spawn(radio_task(coords_send, cmd_send, rx).unwrap());
	spawner.spawn(motor_task(coords_recv, drive).unwrap());
	spawner.spawn(hardware_task(cmd_recv).unwrap());

	
}
