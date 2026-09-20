use core::convert::Infallible;

use embedded_hal::digital::OutputPin;
use embedded_hal_async::spi::SpiDevice;

use embedded_nrf24l01_async::{Configuration, CrcMode, DataRate, NRF24L01, RxMode, StandbyMode, TxMode};

/// Задержка опроса радио (в миллисекундах)
pub const RADIO_POLL_INTERVAL_MS: u64 = 10;
/// Номер радиочастотного канала
const RADIO_CHANNEL: u8 = 110;
/// Адрес
const RADIO_ADDRESS: &[u8; 5] = b"zlo47";

pub type NrfRx<CE, SPI> = RxMode<NRF24L01<<CE as embedded_hal::digital::ErrorType>::Error, CE, SPI>>;
pub type NrfTx<CE, SPI> = TxMode<NRF24L01<<CE as embedded_hal::digital::ErrorType>::Error, CE, SPI>>;
type NrfStandby<CE, SPI> = StandbyMode<NRF24L01<<CE as embedded_hal::digital::ErrorType>::Error, CE, SPI>>;

async fn init_radio<CE, SPI>(
	ce: CE, 
	spi_device: SPI
) -> NrfStandby<CE, SPI> 
where
	CE: OutputPin<Error = Infallible>,
	SPI: SpiDevice,
{
	let mut nrf = NRF24L01::new(ce, spi_device).await.unwrap();

	nrf.set_frequency(RADIO_CHANNEL).await.unwrap();
	nrf.set_auto_retransmit(5, 4).await.unwrap();
	nrf.set_auto_ack(&[true, false, false, false, false, false]).await.unwrap();

	nrf.set_rf(&DataRate::R2Mbps, 0).await.unwrap();
	nrf.set_pipes_rx_enable(&[true, false, false, false, false, false]).await.unwrap();
	nrf.set_pipes_rx_lengths(&[None; 6]).await.unwrap();
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
	let nrf = init_radio(ce, spi_device).await;
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
	let nrf = init_radio(ce, spi_device).await;
	nrf.rx().await.unwrap()
}