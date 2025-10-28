use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use x86_64::instructions::hlt; 
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin::Mutex;
use crate::constants::interrupts::{PIC_1_OFFSET, PIC_2_OFFSET};
use crate::constants::keyboard::DATA_PORT;
use crate::println;

/// Hardware interrupt numbers (after remapping)
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
    // PIC 1 (master) IRQs 2-7
    Cascade,
    COM2,
    COM1,
    LPT2,
    FloppyDisk,
    LPT1,
    // PIC 2 (slave) IRQs 8-15
    RTC = PIC_2_OFFSET,
    ACPI,
    Available1,
    Available2,
    Mouse,
    CoProcessor,
    PrimaryATA,
    SecondaryATA,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

/// Programmable Interrupt Controller (PIC) setup
pub static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // CPU exceptions
        idt.breakpoint.set_handler_fn(breakpoint_handler);

        // Double fault handler with separate stack (IST)
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(crate::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);

        // Hardware interrupts - set handlers for ALL PIC interrupts to avoid triple faults
        idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);
        idt[InterruptIndex::Cascade.as_u8()].set_handler_fn(spurious_interrupt_handler);
        idt[InterruptIndex::COM2.as_u8()].set_handler_fn(spurious_interrupt_handler);
        idt[InterruptIndex::COM1.as_u8()].set_handler_fn(spurious_interrupt_handler);
        idt[InterruptIndex::LPT2.as_u8()].set_handler_fn(spurious_interrupt_handler);
        idt[InterruptIndex::FloppyDisk.as_u8()].set_handler_fn(spurious_interrupt_handler);
        idt[InterruptIndex::LPT1.as_u8()].set_handler_fn(spurious_interrupt_handler);
        idt[InterruptIndex::RTC.as_u8()].set_handler_fn(spurious_interrupt_handler);
        idt[InterruptIndex::ACPI.as_u8()].set_handler_fn(spurious_interrupt_handler);
        idt[InterruptIndex::Available1.as_u8()].set_handler_fn(spurious_interrupt_handler);
        idt[InterruptIndex::Available2.as_u8()].set_handler_fn(spurious_interrupt_handler);
        idt[InterruptIndex::Mouse.as_u8()].set_handler_fn(spurious_interrupt_handler);
        idt[InterruptIndex::CoProcessor.as_u8()].set_handler_fn(spurious_interrupt_handler);
        idt[InterruptIndex::PrimaryATA.as_u8()].set_handler_fn(spurious_interrupt_handler);
        idt[InterruptIndex::SecondaryATA.as_u8()].set_handler_fn(spurious_interrupt_handler);

        idt
    };
}

pub fn init() {
    use crate::println;

    println!("DEBUG: Loading IDT into CPU...");
    IDT.load();
    println!("DEBUG: IDT loaded");

    println!("DEBUG: Initializing PICs...");
    // Initialize and remap the PICs
    unsafe {
        PICS.lock().initialize();

        // Wait for PICs to stabilize - do a few I/O reads
        use x86_64::instructions::port::Port;
        let mut wait_port: Port<u8> = Port::new(0x80);  // Unused port for timing
        for _ in 0..10 {
            wait_port.read();
        }
    }
    println!("DEBUG: PICs initialized");

    // Unmask BOTH timer (IRQ0) and keyboard (IRQ1) for testing
    unsafe {
        use x86_64::instructions::port::Port;
        let mut pic1_data: Port<u8> = Port::new(0x21);

        // Read current mask
        let mask_before = pic1_data.read();
        println!("DEBUG: PIC1 mask BEFORE unmask: {:#04x}", mask_before);

        // Unmask ONLY IRQ0 (timer) for now - keyboard uses polling
        // Keep IRQ1 (keyboard) MASKED so interrupt doesn't interfere with polling
        let new_mask = mask_before & !(1 << 0);  // Only unmask timer
        println!("DEBUG: Writing new mask: {:#04x}", new_mask);
        pic1_data.write(new_mask);

        // Wait for write to complete
        let mut wait_port: Port<u8> = Port::new(0x80);
        for _ in 0..10 {
            wait_port.read();
        }

        // Verify it was written
        let mask_after = pic1_data.read();
        println!("DEBUG: PIC1 mask AFTER unmask: {:#04x}", mask_after);
        println!("DEBUG: Timer (bit 0): {}, Keyboard (bit 1): {}",
                 if (mask_after & 1) == 0 { "UNMASKED" } else { "MASKED" },
                 if (mask_after & 2) == 0 { "UNMASKED" } else { "MASKED" });
    }

    // Enable interrupts globally (sti instruction)
    println!("DEBUG: Calling sti...");
    x86_64::instructions::interrupts::enable();
    println!("DEBUG: sti called, interrupts should be enabled");

    // Check if interrupts are actually enabled
    let enabled = x86_64::instructions::interrupts::are_enabled();
    println!("DEBUG: Interrupts enabled? {}", enabled);
}

/// Initialize IDT and PICs but DO NOT enable interrupts (no sti)
/// This allows pure polling mode while keeping exception handlers available
pub fn init_without_sti() {
    use crate::println;

    println!("DEBUG: Loading IDT into CPU...");
    IDT.load();
    println!("DEBUG: IDT loaded");

    println!("DEBUG: Initializing PICs...");
    // Initialize and remap the PICs
    unsafe {
        PICS.lock().initialize();
    }
    println!("DEBUG: PICs initialized");

    println!("DEBUG: Interrupts NOT enabled (no sti) - using pure polling mode");
}

// Exception handlers
extern "x86-interrupt" fn breakpoint_handler(_stack_frame: InterruptStackFrame) {
    // Use direct VGA write to avoid println! issues in exception context
    unsafe {
        let vga_buffer = 0xb8000 as *mut u8;
        let msg = b"BP!";
        let offset = 320; // Third line
        for (i, &byte) in msg.iter().enumerate() {
            *vga_buffer.offset((offset + i * 2) as isize) = byte;
            *vga_buffer.offset((offset + i * 2 + 1) as isize) = 0x2e; // Yellow on black
        }
    }
}

extern "x86-interrupt" fn double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    println!("EXCEPTION: DOUBLE FAULT - halting");

    loop {
        hlt();
    }
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: x86_64::structures::idt::PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    loop {
        x86_64::instructions::hlt();
    }
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    println!("EXCEPTION: GENERAL PROTECTION FAULT");
    println!("Error Code: {}", error_code);
    println!("{:#?}", stack_frame);
    loop {
        x86_64::instructions::hlt();
    }
}

// Hardware interrupt handlers
extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // DEBUG: Visual indicator that timer interrupt fired
    static mut TIMER_COUNT: u32 = 0;
    unsafe {
        TIMER_COUNT += 1;
        if TIMER_COUNT == 1 {
            // Write directly to VGA buffer
            let vga_buffer = 0xb8000 as *mut u8;
            let msg = b"TIMER!";
            let offset = 160; // Second line
            for (i, &byte) in msg.iter().enumerate() {
                *vga_buffer.offset((offset + i * 2) as isize) = byte;
                *vga_buffer.offset((offset + i * 2 + 1) as isize) = 0x2f; // Green on white
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    // DEBUG: Visual indicator that interrupt fired
    static mut INTERRUPT_COUNT: u32 = 0;
    unsafe {
        INTERRUPT_COUNT += 1;
        if INTERRUPT_COUNT <= 5 {
            // Write directly to VGA buffer to avoid println issues
            let vga_buffer = 0xb8000 as *mut u8;
            let msg = b"INT!";
            for (i, &byte) in msg.iter().enumerate() {
                *vga_buffer.offset((i * 2) as isize) = byte;
                *vga_buffer.offset((i * 2 + 1) as isize) = 0x4f; // White on red
            }
        }
    }

    // Read scancode from keyboard data port
    let mut port = Port::new(DATA_PORT);
    let scancode: u8 = unsafe { port.read() };

    // Queue it for processing in main loop
    crate::keyboard::add_scancode(scancode);

    // Acknowledge interrupt
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

extern "x86-interrupt" fn spurious_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Spurious interrupt - just acknowledge it and return
    // We don't know which interrupt number this is, so acknowledge both PICs
    unsafe {
        PICS.lock().notify_end_of_interrupt(PIC_2_OFFSET);
    }
}
