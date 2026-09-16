use embedded_hal::digital::OutputPin;
use embedded_hal::pwm::SetDutyCycle;

pub enum Side {
    Left,
    Right,
}

pub struct RoverDrive<CH, STBY> 
where
    CH: SetDutyCycle,
    STBY: OutputPin,
{
    // Левый борт
    ch_left_fwd: CH,  // AIN1
    ch_left_rev: CH,  // AIN2
    // Правый борт
    ch_right_fwd: CH, // BIN1
    ch_right_rev: CH, // BIN2
    
    max_duty: u16,      // Трейт SetDutyCycle работает строго с u16 (от 0 до max_duty)

    // Управление питанием/сном драйвера
    stby: STBY,
}

impl<CH, STBY> RoverDrive<CH, STBY>
where
    CH: SetDutyCycle,
    STBY: OutputPin,
{
    /// Создает новый привод ровера из любых каналов ШИМ любого микроконтроллера
    pub fn new(
        ch1_bin2: CH,
        ch2_bin1: CH,
        ch3_ain1: CH,
        ch4_ain2: CH,
        mut stby: STBY,
    ) -> Self {
        // Трейт SetDutyCycle не имеет метода .enable(), 
        // так как включение ШИМ-генератора обычно настраивается глобально в main.rs
        
        // Берем максимальное заполнение (у трейта этот метод возвращает u16)
        let max_duty = ch1_bin2.max_duty_cycle();

        let _ = stby.set_high();

        Self {
            ch_left_fwd: ch3_ain1,
            ch_left_rev: ch4_ain2,
            ch_right_fwd: ch2_bin1,
            ch_right_rev: ch1_bin2,
            max_duty,
            stby
        }
    }

    /// Позволяет при необходимости усыпить драйвер для экономии энергии батареи
   /*pub fn sleep(&mut self) {
        let _ = self.stby.set_low();
    }

    /// Пробуждает драйвер обратно
    pub fn wakeup(&mut self) {
        let _ = self.stby.set_high();
    }*/

    /// Устанавливает скорость для конкретного борта (от -100 до 100)
    pub fn set_speed(&mut self, side: Side, speed: i8) {
        let speed = speed.clamp(-100, 100);
        let safe_max_duty = (self.max_duty as u32 * 80) / 100; // Лимит 80% для 2S батареи TODO: не каждый раз ведь считать! вынести в инициализацию

        // Выбираем нужную пару каналов в зависимости от выбранного борта
        let (ch_fwd, ch_rev) = match side {
            Side::Left => (&mut self.ch_left_fwd, &mut self.ch_left_rev),
            Side::Right => (&mut self.ch_right_fwd, &mut self.ch_right_rev),
        };

        if speed > 0 {
            let duty = (safe_max_duty * speed as u32) / 100;
            let _ = ch_fwd.set_duty_cycle(self.max_duty);
            let _ = ch_rev.set_duty_cycle((self.max_duty as u32 - duty) as u16); // Slow Decay
        } else if speed < 0 {
            let duty = (safe_max_duty * speed.abs() as u32) / 100;
            let _ = ch_fwd.set_duty_cycle((self.max_duty as u32 - duty) as u16);
            let _ = ch_rev.set_duty_cycle(self.max_duty);
        } else {
            let _ = ch_fwd.set_duty_cycle(self.max_duty);
            let _ = ch_rev.set_duty_cycle(self.max_duty);
        }
    }

    // для джойстика удобно
    pub fn arcade_drive(&mut self, move_value: i8, rotate_value: i8) {
        let move_value = move_value.clamp(-100, 100);
        let rotate_value = rotate_value.clamp(-100, 100);

        let left = move_value + rotate_value;
        let right = move_value - rotate_value;

        self.set_speed(Side::Left, left);
        self.set_speed(Side::Right, right);
    }
/*
    // танковый разворот
    pub fn spin_place_left(&mut self, speed: i8) {
        let speed = speed.abs();
        self.set_speed(Side::Left, -speed);
        self.set_speed(Side::Right, speed);
    }*/
}