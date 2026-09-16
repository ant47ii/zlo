pub trait AsyncAdcReader {
    async fn read_raw(&mut self) -> (u16, u16);
}

#[derive(Debug, Clone, Copy)]
pub struct RemoteCommand {
    pub x: i8, // -100...+100
    pub y: i8,
}

pub struct Joystick<R: AsyncAdcReader> {
    reader: R,
    zero_x: u16,
    zero_y: u16,
}

impl<R: AsyncAdcReader> Joystick<R> {

    pub fn new(reader: R) -> Self {
        Self {
            reader,
            zero_x: 127,
            zero_y: 127,
        }
    }

	pub async fn read(&mut self) -> RemoteCommand {
		let (raw_x, raw_y) = self.reader.read_raw().await;
		
		RemoteCommand {
			x: self.process_axis(raw_x, self.zero_x),
			y: self.process_axis(raw_y, self.zero_y),
		}
	}

	pub async fn calibrate(&mut self) {
		let mut sum_x = 0u32;
		let mut sum_y = 0u32;
		const ITERATIONS: u32 = 32;

		for _ in 0..ITERATIONS {
			let (raw_x, raw_y) = self.reader.read_raw().await;
			sum_x += raw_x as u32;
			sum_y += raw_y as u32;
		}

		self.zero_x = (sum_x / ITERATIONS) as u16;
		self.zero_y = (sum_y / ITERATIONS) as u16;
	}

	fn process_axis(&self, raw: u16, zero: u16) -> i8 {
		let delta = raw as i32 - zero as i32;
		
		if delta.abs() < 5 {
			return 0;
		}

		let max_range = if delta > 0 { 
			255 - zero as i32 
		} else { 
			zero as i32 
		};

		if max_range == 0 {
			return 0;
		}

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