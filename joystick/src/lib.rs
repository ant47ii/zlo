#![no_std]
mod joystick;
pub use joystick::*;

cfg_if::cfg_if! {
	if #[cfg(feature = "stm32")] {
		mod joystick_stm32;
		pub use joystick_stm32::*;
	} else if #[cfg(feature = "esp32")] {
		todo!("esp32 не реализовано")
	} else if #[cfg(feature = "ch32")] {
		todo!("ch32 не реализовано")
	}
}