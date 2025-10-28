#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

mod constants;
mod vga_buffer;
mod keyboard;
mod shell;
mod gdt;
mod interrupts;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {
        x86_64::instructions::hlt();
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("DEBUG: Starting DxOS...");

    // Initialize GDT with TSS for double fault protection
    gdt::init();

    // Initialize interrupts (IDT, PICs) but DON'T call sti
    interrupts::init_without_sti();

    //vga_buffer::clear_screen();

    println!("Welcome to DxOS CLI v0.2");
    println!("Type 'help' for available commands.");
    println!("Use UP/DOWN arrows for command history.");
    print!("> ");

    // Main event loop - interrupt-driven (no hlt for testing)
    loop {
        // Process all pending keyboard input from interrupt queue
        while let Some(key) = keyboard::get_key() {
            shell::process_key(key);
        }

        // NO hlt() - just spin to see if interrupts fire
    }
}
