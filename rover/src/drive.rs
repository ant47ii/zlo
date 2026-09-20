use embedded_hal::{digital::OutputPin, pwm::SetDutyCycle};

pub enum Side {
	Left,
	Right,
}

pub struct RoverDrive<CH, STBY> 
where
	CH: SetDutyCycle, 
	STBY: OutputPin
{
	// Левый борт
	ch_left_fwd: CH,  // AIN1
	ch_left_rev: CH,  // AIN2
	// Правый борт
	ch_right_fwd: CH, // BIN1
	ch_right_rev: CH, // BIN2

	stby: STBY,
	
	max_duty: u16,
	safe_max_duty: u32
}

impl<CH, STBY> RoverDrive<CH, STBY>
where
	CH: SetDutyCycle,
	STBY: OutputPin
{
	pub fn new(
		ch1_bin2: CH,
		ch2_bin1: CH,
		ch3_ain1: CH,
		ch4_ain2: CH,
		mut stby: STBY
	) -> Self {
		let max_duty = ch1_bin2.max_duty_cycle();
		let safe_max_duty = (max_duty as u32 * 80) / 100;

		let _ = stby.set_high();

		Self {
			ch_left_fwd: ch3_ain1,
			ch_left_rev: ch4_ain2,
			ch_right_fwd: ch2_bin1,
			ch_right_rev: ch1_bin2,
			stby,
			max_duty,
			safe_max_duty
		}
	}

	/// Устанавливает скорость для конкретного борта (от -100 до 100)
	pub fn set_speed(&mut self, side: Side, speed: i8) {
		let speed = speed.clamp(-100, 100);

		// Выбираем нужную пару каналов в зависимости от выбранного борта
		let (ch_fwd, ch_rev) = match side {
			Side::Left => (&mut self.ch_left_fwd, &mut self.ch_left_rev),
			Side::Right => (&mut self.ch_right_fwd, &mut self.ch_right_rev),
		};

		if speed > 0 {
			let duty = (self.safe_max_duty * speed as u32) / 100;
			let _ = ch_fwd.set_duty_cycle(self.max_duty);
			let _ = ch_rev.set_duty_cycle((self.max_duty as u32 - duty) as u16); // Slow Decay
		} else if speed < 0 {
			let duty = (self.safe_max_duty * speed.unsigned_abs() as u32) / 100;
			let _ = ch_fwd.set_duty_cycle((self.max_duty as u32 - duty) as u16);
			let _ = ch_rev.set_duty_cycle(self.max_duty);
		} else {
			let _ = ch_fwd.set_duty_cycle(self.max_duty);
			let _ = ch_rev.set_duty_cycle(self.max_duty);
		}
	}

	/// Движение
	pub fn arcade_drive(&mut self, move_value: i8, rotate_value: i8) {
		let move_value = move_value.clamp(-100, 100);
		let rotate_value = rotate_value.clamp(-100, 100);

		let left_raw = move_value as i16 + rotate_value as i16;
		let right_raw = move_value as i16 - rotate_value as i16;

		let left = left_raw.clamp(-100, 100) as i8;
		let right = right_raw.clamp(-100, 100) as i8;

		self.set_speed(Side::Left, left);
		self.set_speed(Side::Right, right);
	}

}