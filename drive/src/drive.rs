use embedded_hal::pwm::SetDutyCycle;

pub trait Rover {
	fn set_speed(&mut self, side: Side, speed: i8);
	fn arcade_drive(&mut self, target_move: i16, target_rotate: i16);
}

pub enum Side {
	Left,
	Right,
}

pub struct RoverSide<CH> {
	fwd: CH,
	rev: CH
}

impl<CH> RoverSide<CH> {
	pub fn new(fwd: CH, rev: CH) -> Self {
		Self { fwd, rev }
	}
}

pub struct RoverDrive<CH> 
where
	CH: SetDutyCycle
{
	// Левый борт
	left: RoverSide<CH>,
	// Правый борт
	right : RoverSide<CH>,
	
	max_duty: u16,
	safe_max_duty: u32,

	current_left: i16,
	current_right: i16
}

impl<CH> RoverDrive<CH> 
where
	CH: SetDutyCycle
{
	pub fn new(left: RoverSide<CH>, right: RoverSide<CH>) -> Self {
		let max_duty = left.fwd.max_duty_cycle();
		let safe_max_duty = (max_duty as u32 * 80) / 100;	// TODO: процент передавать параметром

		Self {
			left,
			right,
			max_duty,
			safe_max_duty,
			current_left: 0,
			current_right: 0
		}
	}
}

impl<CH> Rover for RoverDrive<CH>
where
	CH: SetDutyCycle
{
	/// Устанавливает скорость для конкретного борта (от -100 до 100)
	fn set_speed(&mut self, side: Side, speed: i8) {
		let speed = speed.clamp(-100, 100);

		// Выбираем нужную пару каналов в зависимости от выбранного борта
		let (ch_fwd, ch_rev) = match side {
			Side::Left => (&mut self.left.fwd, &mut self.left.rev),
			Side::Right => (&mut self.right.fwd, &mut self.right.rev),
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
	fn arcade_drive(&mut self, target_move: i16, target_rotate: i16) {
		// 1. Ограничиваем входящие значения от джойстика
		let target_move = target_move.clamp(-100, 100);
		let target_rotate = target_rotate.clamp(-100, 100);

		// 2. Считаем сырые целевые значения (они могут выйти за рамки -200..200)
		let raw_left = target_move + target_rotate;
		let raw_right = target_move - target_rotate;

		// 3. ПРОПОРЦИОНАЛЬНОЕ МАСШТАБИРОВАНИЕ (Приоритет поворота)
		// Находим максимальный модуль скорости среди обоих бортов
		let max_val = raw_left.abs().max(raw_right.abs());

		let (target_left, target_right) = if max_val > 100 {
			// Если вылетели за 100%, сжимаем оба борта обратно к 100%, сохраняя пропорцию руления.
			// Теперь при move=100 и rotate=100 мы получим left=100, right=-100 вместо (100, 0)
			(
				(raw_left * 100) / max_val,
				(raw_right * 100) / max_val
			)
		} else {
			(raw_left, raw_right)
		};

		// 4. КОНСТАНТЫ ЛИНЕЙНОГО ШАГА (Скорректированы под интервал 10 мс)
		const ACCEL_STEP: i16 = 8;   // Разгон от 0 до 100 займет ~125 мс
		const BRAKE_STEP: i16 = 40;  // Торможение от 100 до 0 займет ~25 мс

		// Вспомогательная функция расчета шага
		#[inline(always)]
		fn calc_next_speed(current: i16, target: i16, accel: i16, brake: i16) -> i16 {
			if current == target {
				return current;
			}

			// Проверяем, разгоняемся ли мы: знаки совпадают и цель дальше от нуля, чем текущая
			let is_accel = (target > 0 && current >= 0 && target > current) 
						|| (target < 0 && current <= 0 && target < current);
			
			let step = if is_accel { accel } else { brake };

			if current < target {
				current.saturating_add(step).min(target)
			} else {
				current.saturating_sub(step).max(target)
			}
		}

		// 5. Применяем линейный фильтр к каждому борту отдельно
		self.current_left = calc_next_speed(self.current_left, target_left, ACCEL_STEP, BRAKE_STEP);
		self.current_right = calc_next_speed(self.current_right, target_right, ACCEL_STEP, BRAKE_STEP);

		let left_i8 = self.current_left as i8;
		let right_i8 = self.current_right as i8;

		self.set_speed(Side::Left, left_i8);
		self.set_speed(Side::Right, right_i8);
	}
}

