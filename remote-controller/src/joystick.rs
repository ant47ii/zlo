use embassy_stm32::{
	Peri, 
	adc::{Adc, AdcConfig, AnyAdcChannel, Averaging, Resolution, SampleTime}, 
	dma::ChannelInstance, peripherals::ADC1
};
use crate::Irqs;

#[derive(Debug, defmt::Format, Clone, Copy)]
pub struct RemoteCommand {
	pub x: i8,	// -100...+100
	pub y: i8,
}

pub struct Joystick<'d, T: embassy_stm32::adc::RxDma<ADC1> + ChannelInstance > {
	axis_x: AnyAdcChannel<'d, ADC1>,
	axis_y: AnyAdcChannel<'d, ADC1>,
	adc: Adc<'d, ADC1>,
	dma: Peri<'d, T>,
	buffer: [u16; 2],
	zero_x: u16,
	zero_y: u16,
}

impl<'d, T: embassy_stm32::adc::RxDma<ADC1> + ChannelInstance> Joystick<'d, T>
where
	Irqs: embassy_stm32::interrupt::typelevel::Binding<
		<T as ChannelInstance>::Interrupt, 
		embassy_stm32::dma::InterruptHandler<T>
	>
{
	pub fn new(axis_x: AnyAdcChannel<'d, ADC1>, axis_y: AnyAdcChannel<'d, ADC1>, adc_periph: Peri<'d, ADC1>, dma: Peri<'d, T>) -> Self {
		// adc должен создаваться в main
		let adc_config = AdcConfig {
				averaging: Some(Averaging::Samples16),
				resolution: Some(Resolution::BITS8),
				..Default::default()
		};

		let adc = Adc::new_with_config(adc_periph, adc_config);
			
		Self {
			axis_x,
			axis_y,
			adc,
			dma,
			buffer: [0u16; 2],
			zero_x: 127,
			zero_y: 127
		}
	}

	// 0...127...255
	async fn read_raw(&mut self) -> (u16, u16) {
		let cycles = SampleTime::CYCLES160_5;

		self.adc.read(
			self.dma.reborrow(),
			Irqs, 
			[
				(&mut self.axis_x, cycles),
				(&mut self.axis_y, cycles),
			].into_iter(), 
			&mut self.buffer
		).await;

		(
			self.buffer[0],
			self.buffer[1]
		)
	}

	pub async fn read(&mut self) -> RemoteCommand {
		let (raw_x, raw_y) = self.read_raw().await;

		RemoteCommand {
			x: self.process_axis(raw_x, self.zero_x),
			y: self.process_axis(raw_y, self.zero_y)
		}
	}

	pub async fn calibrate(&mut self) {
		let mut sum_x = 0u32;
		let mut sum_y = 0u32;
		const ITERATIONS: u32 = 32;

		for _ in 0..ITERATIONS {
			let (raw_x, raw_y) = self.read_raw().await;
			sum_x += raw_x as u32;
			sum_y += raw_y as u32;
		}

		self.zero_x = (sum_x / ITERATIONS) as u16;
		self.zero_y = (sum_y / ITERATIONS) as u16;
	}

	fn process_axis(&self, raw: u16, zero: u16) -> i8 {
		// Измеряем отклонение от центра (в диапазоне 0..255)
		let delta = raw as i32 - zero as i32;

		// МЕРТВАЯ ЗОНА для 8 бит: если прыжки в пределах ±4 единиц, возвращаем 0.
		// Если значения всё ещё будут слегка прыгать, замените 4 на 5 или 6.
		if delta.abs() < 5 {
			return 0;
		}

		// Вычисляем доступный диапазон хода
		let max_range = if delta > 0 {
			255 - zero as i32
		} else {
			zero as i32
		};

		if max_range == 0 { 
			return 0; 
		}

		// Масштабируем в диапазон [-100; 100] для отправки
		let scaled = (delta * 100) / max_range;
		
		scaled.clamp(-100, 100) as i8
	}
}

pub fn apply_joystick_expo(raw_input: i8, expo_factor: f32) -> i8 {	// i8
	// 1. Фильтруем мертвую зону (Deadzone) вокруг нуля
	if raw_input.abs() < 10 {
		return 0;
	}

	let input_f = raw_input as f32;
	let k_cubic = expo_factor / 10_000.0;
	let output_f = (1.0 - expo_factor) * input_f + k_cubic * (input_f * input_f * input_f);
	output_f.clamp(-100.0, 100.0) as i8
}