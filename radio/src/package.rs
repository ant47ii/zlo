use core::convert::Infallible;

use embedded_nrf24l01_async::Configuration;
use serde::{Deserialize, Serialize};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, DynamicSender};
use embassy_sync::watch::{DynSender, Watch};

use embedded_hal::digital::OutputPin;
use embedded_hal_async::spi::SpiDevice;

use embassy_time::{Duration, Timer, with_timeout};

use defmt::{error};

use crate::{NrfRx, NrfTx};

// один отправитель, несколько подписчиков
// хранит только одно состояние
// хранит только самое свежее состояние
pub static COORDS_WATCH: Watch<CriticalSectionRawMutex, JoystickCoords, 1> = Watch::new();		// 1 - максимальнрое количество подписчиков

// один отправитель, один подписчик
// максимум читает одна таска
// хранит все сообщения, новые данные встают в конец очереди
pub static CMD_CHANNEL: Channel<CriticalSectionRawMutex, JoystickCommand, 4> = Channel::new();		// 4 - кольцевой буфер

/// Джойстик
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct JoystickCoords {
	pub x: i8,
	pub y: i8,
}

/// Команды
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum JoystickCommand {

}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum RadioPackage {
	Coords(JoystickCoords),
	Command(JoystickCommand)
}

/// Отправить пакет
pub async fn send<CE, SPI>(
	tx: &mut NrfTx<CE, SPI>, 
	radio_package: RadioPackage, 
	buffer: &mut [u8; 16]
) -> bool
where
	CE: OutputPin<Error = Infallible>,
	SPI: SpiDevice,
{
	if !tx.can_send().await.unwrap() {
		return false;
	}

	let packet = postcard::to_slice(&radio_package, buffer).unwrap();
	if let Err(_) = tx.send(&packet).await {
		return false;
	}

	let result = with_timeout(Duration::from_millis(75), async { // TODO: const
		loop {
			match tx.poll_send().await {
				Ok(ack_received) => return ack_received,
				Err(_) =>  Timer::after_micros(500).await
			}
		}
	}).await;

	match result {
		Ok(ack_status) => ack_status,
		Err(_timeout_error) => false
	}
}

async fn parse_and_route_packet(
	payload: &[u8],
	coords_sender: &DynSender<'static, JoystickCoords>,
	cmd_sender: &DynamicSender<'static, JoystickCommand>
) {
	match postcard::from_bytes::<RadioPackage>(payload) {
		Ok(RadioPackage::Coords(coords)) => {
			coords_sender.send(coords);
		}
		Ok(RadioPackage::Command(command)) => {
			cmd_sender.send(command).await;
		}
		Err(_) => {
		}
	}
}

// TODO: не та зона ответственности
pub async fn run_radio_reader<CE, SPI>(
	mut rx: NrfRx<CE, SPI>,
	coords_sender: &DynSender<'static, JoystickCoords>,
	cmd_sender: &DynamicSender<'static, JoystickCommand>
)
where
	CE: OutputPin,
	SPI: SpiDevice,
{
	loop {
		match rx.can_read().await {
			Ok(Some(_)) => {
				match rx.read().await {
					Ok(payload) => {
						parse_and_route_packet(&payload, &coords_sender, &cmd_sender).await;
					}
					Err(e) => {
						error!("Ошибка чтения пакета из FIFO: {:?}", defmt::Debug2Format(&e));
					}
				}
				rx.clear_interrupts().await.unwrap();
			}
			Ok(None) => {}
			Err(e) => {
				error!("Ошибка опроса регистра статуса nRF: {:?}", defmt::Debug2Format(&e));
			}
		}
		
		Timer::after_millis(10).await;	// TODO: const
	}
}