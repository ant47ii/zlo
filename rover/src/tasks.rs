use embedded_hal::{digital::OutputPin, pwm::SetDutyCycle};
use radio::{JoystickCommand, JoystickCoords};
use embassy_sync::{channel::{DynamicReceiver, DynamicSender}, watch::{DynReceiver, DynSender}};
use embassy_time::{Duration, with_timeout};

use crate::drive::{RoverDrive};

// унести?
type Stm32SpiBus = embassy_stm32::spi::Spi<'static, embassy_stm32::mode::Async, embassy_stm32::spi::mode::Master>;
type Stm32CePin = embassy_stm32::gpio::Output<'static>;
type Stm32CsPin = embassy_stm32::gpio::Output<'static>;
type Stm32SpiDevice = embedded_hal_bus::spi::ExclusiveDevice<Stm32SpiBus, Stm32CsPin, embassy_time::Delay>;
type Stm32NrfRx = radio::NrfRx<Stm32CePin, Stm32SpiDevice>;

type Stm32Rover = RoverDrive<
    embassy_stm32::timer::simple_pwm::SimplePwmChannel<'static, embassy_stm32::peripherals::TIM1>,
    embassy_stm32::gpio::Output<'static>
>;

/// Слушает радио
#[embassy_executor::task]
pub async fn radio_task(
    coords_sender: DynSender<'static, JoystickCoords>,
    cmd_sender: DynamicSender<'static, JoystickCommand>,
	rx: Stm32NrfRx
) {
    radio::run_radio_reader(rx, &coords_sender, &cmd_sender).await;
}

/// Управляет моторами
#[embassy_executor::task]
pub async fn motor_task(
    coords_receiver: DynReceiver<'static, JoystickCoords>, 
    drive: Stm32Rover
) {
    run_rover_control(coords_receiver, drive).await;
}

async fn run_rover_control<CH, STBY>(
    mut coords_receiver: DynReceiver<'static, JoystickCoords>, 
    mut drive: RoverDrive<CH, STBY>
)
where
    CH: SetDutyCycle,
    STBY: OutputPin,
{
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
pub async fn hardware_task(cmd_receiver: DynamicReceiver<'static, JoystickCommand>) {
	loop {
		let cmd = cmd_receiver.receive().await;
		match cmd {
            
        }
	}
}