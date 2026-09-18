#![no_std]
mod joystick;
pub use joystick::*;

#[cfg(feature = "stm32")]
mod joystick_stm32;
pub use joystick_stm32::*;