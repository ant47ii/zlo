use core::convert::Infallible;

use embedded_nrf24l01_async::{Configuration};
use serde::{Deserialize, Serialize};

use embedded_hal::digital::OutputPin;
use embedded_hal_async::spi::SpiDevice;

use embassy_time::{Duration, Timer, with_timeout};

use crate::{NrfRx, NrfTx};

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
	// TODO: добавить рукопожатие?
}

/// Отправить пакет
pub async fn send<CE, SPI>(
	tx: &mut NrfTx<CE, SPI>, 
	radio_package: RadioPackage, 
	buffer: &mut [u8; 16]
) -> bool
where
	CE: OutputPin<Error = Infallible>,
	SPI: SpiDevice
{
	if !tx.can_send().await.unwrap() {
		return false;
	}

	buffer.fill(0);
	let packet = postcard::to_slice(&radio_package, buffer).unwrap();
	if let Err(_) = tx.send(packet).await {
		return false;
	}

	let result = with_timeout(Duration::from_millis(5), async { // TODO: const
		loop {
			match tx.poll_send().await {
				Ok(ack_received) => return ack_received,
				Err(_) => Timer::after_micros(0).await
			}
		}
	}).await;

	match result {
		Ok(ack_status) => ack_status,
		Err(_timeout_error) => { 
			let _ = tx.flush_tx().await;
			let _ = tx.clear_interrupts().await;
			false
		}
	}
}

/// прочитать эфир
pub async fn read<CE, SPI>(
	rx: &mut NrfRx<CE, SPI>
) -> Result<Option<RadioPackage>, &'static str> 
where
	CE: OutputPin,
	SPI: SpiDevice 
{
	match rx.can_read().await {
		Ok(Some(_)) => {			
			match rx.read().await {
				Ok(payload) => {
					let radio_package = postcard::from_bytes::<RadioPackage>(&payload);
					Ok(radio_package.ok())
				}
				Err(_) => {
					return Err("Ошибка чтения пакета из FIFO");
				}
			}
		}
		Ok(None) => { Ok(None) }
		Err(_) => {
			Err("Ошибка опроса регистра статуса nRF")
		}
	}
}

pub async fn clear_interrupts<CE, SPI>(rx: &mut NrfRx<CE, SPI>) 
where
	CE: OutputPin,
	SPI: SpiDevice 
{
	let _ = rx.clear_interrupts().await;
}