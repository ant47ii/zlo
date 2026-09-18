use crate::joystick::AsyncAdcReader;
use embassy_stm32::{
	adc::{Adc, AnyAdcChannel, SampleTime, RxDma, Instance, BasicAdcRegs},
	dma::ChannelInstance,
	Peri,
};

pub struct Stm32JoystickAdc<'d, ADC: Instance, T: RxDma<ADC> + ChannelInstance, I> {
	axis_x: AnyAdcChannel<'d, ADC>,
	axis_y: AnyAdcChannel<'d, ADC>,
	adc: Adc<'d, ADC>,
	dma: Peri<'d, T>,
	irq: I,
	buffer: [u16; 2],
}

impl<'d, ADC: Instance, T: RxDma<ADC> + ChannelInstance, I> Stm32JoystickAdc<'d, ADC, T, I> {
	pub fn new(
		axis_x: AnyAdcChannel<'d, ADC>,
		axis_y: AnyAdcChannel<'d, ADC>,
		adc: Adc<'d, ADC>,
		dma: Peri<'d, T>,
		irq: I,
	) -> Self {
		Self {
			axis_x,
			axis_y,
			adc,
			dma,
			irq,
			buffer: [0u16; 2],
		}
	}
}

impl<'d, ADC: Instance, T: RxDma<ADC> + ChannelInstance, I> AsyncAdcReader for Stm32JoystickAdc<'d, ADC, T, I>
where
	<ADC::Regs as BasicAdcRegs>::SampleTime: From<SampleTime> + Into<SampleTime> + Copy,
	I: embassy_stm32::interrupt::typelevel::Binding<
		<T as ChannelInstance>::Interrupt, 
		embassy_stm32::dma::InterruptHandler<T>
	>
{
	async fn read_raw(&mut self) -> (u16, u16) {
		let cycles: <ADC::Regs as BasicAdcRegs>::SampleTime = SampleTime::CYCLES160_5.into();
		
		self.adc.read(
			self.dma.reborrow(),
			self.irq,
			[
				(&mut self.axis_x, cycles), 
				(&mut self.axis_y, cycles)
			].into_iter(),
			&mut self.buffer,
		).await;

		(self.buffer[0], self.buffer[1])
	}
}