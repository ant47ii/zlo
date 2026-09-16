#![no_std]

use core::convert::Infallible;

use defmt::{error, info};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel/*, DynamicSender*/};
use embassy_sync::watch::{DynSender, Watch};

use embassy_time::{Duration, Timer, with_timeout};
use embedded_hal::digital::OutputPin;
use embedded_hal_async::spi::SpiDevice;

use serde::{Deserialize, Serialize};

use embedded_nrf24l01_async::{Configuration, CrcMode, DataRate, NRF24L01, RxMode, StandbyMode, TxMode};

// ==========================================
//          КОНФИГУРАЦИЯ УСТРОЙСТВА
// ==========================================
// Радио-настройки
pub const RADIO_CHANNEL: u8 = 110;				// Номер радиочастотного канала
pub const RADIO_POLL_INTERVAL_MS: u64 = 20;		// Задержка опроса радио (в миллисекундах)
pub const RADIO_ADDRESS: &[u8; 5] = b"zlo47";	// Настройки адресации (строковые константы)

pub type NrfRx<CE, SPI> = RxMode<NRF24L01<<CE as embedded_hal::digital::ErrorType>::Error, CE, SPI>>;
pub type NrfTx<CE, SPI> = TxMode<NRF24L01<<CE as embedded_hal::digital::ErrorType>::Error, CE, SPI>>;
type NrfStandby<CE, SPI> = StandbyMode<NRF24L01<<CE as embedded_hal::digital::ErrorType>::Error, CE, SPI>>;


/// Джойстик
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct JoystickCoords {
	pub x: i8,
	pub y: i8,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum RadioCommand {
	Coords(JoystickCoords),
	SetHeadlights(bool),
	SetCameraAngle(u8),
}

// один отправитель, несколько подписчиков
// хранит только одно состояние
// хранит только самое свежее состояние
pub static COORDS_WATCH: Watch<CriticalSectionRawMutex, JoystickCoords, 1> = Watch::new();		// 1 - максимальнрое количество подписчиков

// один отправитель, один подписчик
// максимум читает одна таска
// хранит все сообщения, новые данные встают в конец очереди
pub static CMD_CHANNEL: Channel<CriticalSectionRawMutex, RadioCommand, 4> = Channel::new();		// 4 - кольцевой буфер

async fn setup_radio_common<CE, SPI>(
	ce: CE, 
	spi_device: SPI
) -> NrfStandby<CE, SPI> 
where
	CE: OutputPin<Error = Infallible>,
	SPI: SpiDevice,
{
	let mut nrf = NRF24L01::new(ce, spi_device).await.unwrap();

	match nrf.device().is_connected().await {
		Ok(b) => info!("nrf is {}", b),
		Err(_) => error!("nrf disconnected"),
	}

	nrf.set_frequency(RADIO_CHANNEL).await.unwrap();
	nrf.set_auto_retransmit(5, 4).await.unwrap();
	nrf.set_auto_ack(&[true, false, false, false, false, false]).await.unwrap();

	nrf.set_rf(&DataRate::R2Mbps, 0).await.unwrap();
	nrf.set_pipes_rx_enable(&[true, false, false, false, false, false]).await.unwrap();
	nrf.set_pipes_rx_lengths(&[None; 6]).await.unwrap();
	//nrf.set_pipes_rx_lengths(&[Some(4), None, None, None, None, None]).await.unwrap();
	nrf.set_crc(CrcMode::TwoBytes).await.unwrap();
	
	nrf.set_rx_addr(0, RADIO_ADDRESS).await.unwrap();
	nrf.set_tx_addr(RADIO_ADDRESS).await.unwrap();
	
	nrf.flush_rx().await.unwrap();
	nrf.flush_tx().await.unwrap();
	nrf.clear_interrupts().await.unwrap();

	nrf
}

/// Инициализация передатчика
pub async fn init_tx_radio<CE, SPI>(
	ce: CE, 
	spi_device: SPI
) -> NrfTx<CE, SPI> 
where
	CE: OutputPin<Error = Infallible>,
	SPI: SpiDevice,
{
	let nrf = setup_radio_common(ce, spi_device).await;
	nrf.tx().await.unwrap()
}

/// Инициализация приёмника
pub async fn init_rx_radio<CE, SPI>(
	ce: CE, 
	spi_device: SPI
) -> NrfRx<CE, SPI> 
where
	CE: OutputPin<Error = Infallible>,
	SPI: SpiDevice,
{
	let nrf = setup_radio_common(ce, spi_device).await;
	nrf.rx().await.unwrap()
}

/// Отправить команду
pub async fn send<CE, SPI>(
	tx: &mut NrfTx<CE, SPI>, 
	command: RadioCommand, 
	buffer: &mut [u8; 16]
) -> bool
where
	CE: OutputPin<Error = Infallible>,
	SPI: SpiDevice,
{
	if !tx.can_send().await.unwrap() {
		return false;
	}

	let packet = postcard::to_slice(&command, buffer).unwrap();
	if let Err(_) = tx.send(&packet).await {
		return false;
	}

	let result = with_timeout(Duration::from_millis(75), async {
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
	//cmd_sender: &DynamicSender<'static, RadioCommand>,
) {
	match postcard::from_bytes::<RadioCommand>(payload) {
		Ok(RadioCommand::Coords(coords)) => {
			coords_sender.send(coords);
		}
		Ok(RadioCommand::SetHeadlights(on)) => {
			//cmd_sender.send(RadioCommand::SetHeadlights(on));
		}
		Ok(RadioCommand::SetCameraAngle(angle)) => {
			//cmd_sender.send(RadioCommand::SetCameraAngle(angle));
		}
		Err(_) => {
			// Ошибка десериализации: пакет поврежден или не совпадает протокол
		}
	}
}

pub async fn run_radio_reader<CE, SPI>(
	mut rx: NrfRx<CE, SPI>,
	coords_sender: DynSender<'static, JoystickCoords>,
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
						parse_and_route_packet(&payload, &coords_sender).await;
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
		
		Timer::after_millis(10).await;
	}
}