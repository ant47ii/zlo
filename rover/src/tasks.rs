use defmt::info;
use radio::{JoystickCommand, JoystickCoords};
use embassy_time::{Duration, Timer, with_timeout};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, DynamicReceiver, DynamicSender};
use embassy_sync::watch::{Watch, DynReceiver, DynSender};

use crate::drive::{RoverDrive};

// унести?
type Stm32SpiBus = embassy_stm32::spi::Spi<'static, embassy_stm32::mode::Async, embassy_stm32::spi::mode::Master>;
type Stm32CePin = embassy_stm32::gpio::Output<'static>;
type Stm32CsPin = embassy_stm32::gpio::Output<'static>;
type Stm32SpiDevice = embedded_hal_bus::spi::ExclusiveDevice<Stm32SpiBus, Stm32CsPin, embassy_time::Delay>;
type Stm32NrfRx = radio::NrfRx<Stm32CePin, Stm32SpiDevice>;

type Stm32Rover = RoverDrive<
	embassy_stm32::timer::simple_pwm::SimplePwmChannel<'static, embassy_stm32::peripherals::TIM1>
>;

// один отправитель, несколько подписчиков
// хранит только одно состояние
// хранит только самое свежее состояние
pub static COORDS_WATCH: Watch<CriticalSectionRawMutex, radio::JoystickCoords, 1> = Watch::new();		// 1 - максимальнрое количество подписчиков

// один отправитель, один подписчик
// максимум читает одна таска
// хранит все сообщения, новые данные встают в конец очереди
pub static CMD_CHANNEL: Channel<CriticalSectionRawMutex, radio::JoystickCommand, 4> = Channel::new();		// 4 - кольцевой буфер

/// Слушает радио
#[embassy_executor::task]
pub async fn radio_task(
	coords_sender: DynSender<'static, JoystickCoords>,
	cmd_sender: DynamicSender<'static, JoystickCommand>,
	mut rx: Stm32NrfRx
) 
{
	info!("Задание radio запущено");
	loop {
		match radio::read(&mut rx).await {
			Ok(radio_package) => {			
				if radio_package.is_some() {
					match radio_package.unwrap() {
							radio::RadioPackage::Coords(coords) => coords_sender.send(coords),
							radio::RadioPackage::Command(command) => cmd_sender.send(command).await,
						}
				}
			},
			Err(_) => {}
		}

		Timer::after_millis(10).await;	// TODO: const
	}
}

/// Управляет моторами
#[embassy_executor::task]
pub async fn motor_task(
	mut coords_receiver: DynReceiver<'static, JoystickCoords>, 
	mut drive: Stm32Rover
) 
{
	info!("Задание motor запущено");
	loop {
		match with_timeout(Duration::from_millis(360), coords_receiver.changed()).await { // TODO: const
			Ok(coords) => {
				drive.arcade_drive(coords.x, coords.y);
			}
			Err(_timeout) => {
				drive.arcade_drive(0, 0);
			}
		}
	}
}

#[embassy_executor::task]
pub async fn hardware_task(
	cmd_receiver: DynamicReceiver<'static, JoystickCommand>
) 
{
	info!("Задание hardware запущено");
	loop {
		let cmd = cmd_receiver.receive().await;
		match cmd {
			
		}
	}
}