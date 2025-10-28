use spin::Mutex;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use crate::println;

/// Scancode buffer for interrupt-driven keyboard input
const SCANCODE_QUEUE_SIZE: usize = 16;

struct ScancodeQueue {
    buffer: [u8; SCANCODE_QUEUE_SIZE],
    read_pos: usize,
    write_pos: usize,
}

impl ScancodeQueue {
    const fn new() -> Self {
        ScancodeQueue {
            buffer: [0; SCANCODE_QUEUE_SIZE],
            read_pos: 0,
            write_pos: 0,
        }
    }

    fn push(&mut self, scancode: u8) -> Result<(), ()> {
        let next_write = (self.write_pos + 1) % SCANCODE_QUEUE_SIZE;
        if next_write == self.read_pos {
            return Err(()); // Queue full
        }
        self.buffer[self.write_pos] = scancode;
        self.write_pos = next_write;
        Ok(())
    }

    fn pop(&mut self) -> Option<u8> {
        if self.read_pos == self.write_pos {
            return None; // Queue empty
        }
        let scancode = self.buffer[self.read_pos];
        self.read_pos = (self.read_pos + 1) % SCANCODE_QUEUE_SIZE;
        Some(scancode)
    }
}

static SCANCODE_QUEUE: Mutex<ScancodeQueue> = Mutex::new(ScancodeQueue::new());
static KEYBOARD_DECODER: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
    Mutex::new(Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::Ignore,
    ));

/// Called from interrupt handler to add a scancode to the queue
pub fn add_scancode(scancode: u8) {
    if let Err(_) = SCANCODE_QUEUE.lock().push(scancode) {
        println!("WARNING: scancode queue full; dropping keyboard input");
    }
}

/// Get decoded key events from keyboard port (POLLING MODE)
pub fn get_key() -> Option<DecodedKey> {
    use x86_64::instructions::port::Port;
    use crate::constants::keyboard::DATA_PORT;

    let mut port = Port::new(DATA_PORT);
    let mut decoder = KEYBOARD_DECODER.lock();

    // Poll the keyboard status register
    let mut status_port = Port::<u8>::new(0x64);
    let status = unsafe { status_port.read() };

    // Check if output buffer is full (bit 0)
    if (status & 0x01) != 0 {
        // Read scancode from data port
        let scancode = unsafe { port.read() };

        // Decode it
        if let Ok(Some(key_event)) = decoder.add_byte(scancode) {
            if let Some(key) = decoder.process_keyevent(key_event) {
                return Some(key);
            }
        }
    }

    None
}

/// Send reset command to keyboard controller (for reboot)
pub fn reset_cpu() -> ! {
    use x86_64::instructions::port::Port;
    use crate::constants::keyboard::{STATUS_COMMAND_PORT, CMD_RESET_CPU};

    unsafe {
        let mut port = Port::<u8>::new(STATUS_COMMAND_PORT);
        port.write(CMD_RESET_CPU);
    }

    loop {
        x86_64::instructions::hlt();
    }
}
