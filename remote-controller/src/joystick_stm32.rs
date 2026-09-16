use crate::joystick;

use embassy_stm32::{
    adc::{Adc, AnyAdcChannel, SampleTime, RxDma},
    dma::ChannelInstance,
    peripherals::ADC1,
    Peri,
};

pub struct Stm32JoystickAdc<'d, T: RxDma<ADC1> + ChannelInstance> {
    axis_x: AnyAdcChannel<'d, ADC1>,
    axis_y: AnyAdcChannel<'d, ADC1>,
    adc: Adc<'d, ADC1>,
    dma: Peri<'d, T>,
    buffer: [u16; 2],
}

impl<'d, T: RxDma<ADC1> + ChannelInstance> Stm32JoystickAdc<'d, T> {
    pub fn new(
        axis_x: AnyAdcChannel<'d, ADC1>,
        axis_y: AnyAdcChannel<'d, ADC1>,
        adc: Adc<'d, ADC1>,
        dma: Peri<'d, T>,
    ) -> Self {
        Self {
            axis_x,
            axis_y,
            adc,
            dma,
            buffer: [0u16; 2],
        }
    }
}

impl<'d, T: RxDma<ADC1> + ChannelInstance> joystick::AsyncAdcReader for Stm32JoystickAdc<'d, T> 
where
    crate::Irqs: embassy_stm32::interrupt::typelevel::Binding<
        <T as ChannelInstance>::Interrupt,
        embassy_stm32::dma::InterruptHandler<T>,
    >,
{
    async fn read_raw(&mut self) -> (u16, u16) {
        let cycles = SampleTime::CYCLES160_5;
        
        self.adc.read(
            self.dma.reborrow(),
            crate::Irqs,
            [
                (&mut self.axis_x, cycles), 
                (&mut self.axis_y, cycles)
            ].into_iter(),
            &mut self.buffer,
        ).await;

        (
            self.buffer[0], 
            self.buffer[1]
        )
    }
}