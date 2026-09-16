#![no_std]

use core::convert::Infallible;

use defmt::{error, info};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel/*, DynamicSender*/};
use embassy_sync::watch::{DynSender, Watch};


use embassy_time::Timer;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::spi::SpiDevice;

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
#[derive(Clone, Copy, Debug, PartialEq)]	// PartialEq важен для Watch
pub struct JoystickCoords {
	pub val1: i8,
	pub val2: i8,
}

// Команды
#[derive(Clone, Copy, Debug)]
pub enum RadioCommand {
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

async fn parse_and_route_packet(
	payload: &[u8],
	coords_sender: &DynSender<'static, JoystickCoords>,
	//cmd_sender: &DynamicSender<'static, RadioCommand>,
) {
	match payload {
		// [Тип 1, X, Y, ..] -> Координаты джойстика
		&[1, x, y, ..] => {
			info!("команда");
			coords_sender.send(JoystickCoords { val1: x as i8, val2: y as i8 });
		}
		
		// [Тип 2, ИД 1 (Фары), Статус, ..]
		/*&[2, 1, status, ..] => {
			cmd_sender.send(RadioCommand::SetHeadlights(status != 0)).await;
		}
		
		// [Тип 2, ИД 2 (Камера), Угол, ..]
		&[2, 2, angle, ..] => {
			cmd_sender.send(RadioCommand::SetCameraAngle(angle)).await;
		}*/

		// Пакет не подошел ни под одно правило протокола
		_ => {
			/*error!("Получен неизвестный или битый пакет. Длина: {}, первый байт: {}", 
				payload.len(), 
				if !payload.is_empty() { payload[0] } else { 0 }
			);*/
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