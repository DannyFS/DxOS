/// System-wide constants to avoid magic numbers

/// VGA text mode constants
pub mod vga {
    /// VGA text buffer physical address
    pub const BUFFER_ADDR: usize = 0xb8000;

    /// VGA text mode dimensions
    pub const BUFFER_HEIGHT: usize = 25;
    pub const BUFFER_WIDTH: usize = 80;

    /// VGA control ports
    pub const COMMAND_PORT: u16 = 0x3D4;
    pub const DATA_PORT: u16 = 0x3D5;

    /// Cursor control registers
    pub const CURSOR_START_REG: u8 = 0x0A;
    pub const CURSOR_END_REG: u8 = 0x0B;
    pub const CURSOR_LOCATION_HIGH: u8 = 0x0E;
    pub const CURSOR_LOCATION_LOW: u8 = 0x0F;
}

/// PS/2 Keyboard controller constants
pub mod keyboard {
    /// PS/2 keyboard data port
    pub const DATA_PORT: u16 = 0x60;

    /// PS/2 keyboard status/command port
    pub const STATUS_COMMAND_PORT: u16 = 0x64;

    /// Status register bit flags
    pub const STATUS_OUTPUT_BUFFER_FULL: u8 = 0x01;

    /// Command to reset CPU via keyboard controller
    pub const CMD_RESET_CPU: u8 = 0xFE;
}

/// Interrupt constants
pub mod interrupts {
    /// PIC (Programmable Interrupt Controller) offset
    /// We remap PIC interrupts to start at 32 to avoid conflicts with CPU exceptions
    pub const PIC_1_OFFSET: u8 = 32;
    pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;
}
