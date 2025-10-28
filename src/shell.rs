use core::str;
use pc_keyboard::DecodedKey;
use crate::{print, println};

const LINE_BUF_LEN: usize = 128;
const HISTORY_SIZE: usize = 10;

static mut LINE_BUF: [u8; LINE_BUF_LEN] = [0; LINE_BUF_LEN];
static mut LINE_LEN: usize = 0;

static mut HISTORY: [[u8; LINE_BUF_LEN]; HISTORY_SIZE] = [[0; LINE_BUF_LEN]; HISTORY_SIZE];
static mut HISTORY_LENS: [usize; HISTORY_SIZE] = [0; HISTORY_SIZE];
static mut HISTORY_INDEX: usize = 0;
static mut HISTORY_COUNT: usize = 0;
static mut HISTORY_BROWSE_INDEX: Option<usize> = None;

fn prompt() {
    print!("> ");
}

/// Command function type
type CommandFn = fn(&[&str]);

/// Command registry entry
struct Command {
    name: &'static str,
    help: &'static str,
    func: CommandFn,
}

/// Command dispatch table - add new commands here
const COMMANDS: &[Command] = &[
    Command {
        name: "help",
        help: "Display this help message",
        func: cmd_help,
    },
    Command {
        name: "echo",
        help: "Echo arguments to the screen",
        func: cmd_echo,
    },
    Command {
        name: "clear",
        help: "Clear the screen",
        func: cmd_clear,
    },
    Command {
        name: "reboot",
        help: "Reboot the system",
        func: cmd_reboot,
    },
    Command {
        name: "history",
        help: "Show command history",
        func: cmd_history,
    },
];

/// Find command by name
fn find_command(name: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|cmd| cmd.name == name)
}

/// Called from main when a key is decoded
pub fn process_key(key: DecodedKey) {
    match key {
        DecodedKey::Unicode(c) => match c {
            '\n' => {
                let cmd = get_line();
                println!("");
                if !cmd.is_empty() {
                    add_to_history(cmd);
                    execute_command(cmd);
                }
                prompt();
            }
            '\u{8}' | '\u{7f}' => {
                backspace();
            }
            c => {
                push_char(c);
            }
        },
        DecodedKey::RawKey(raw) => {
            use pc_keyboard::KeyCode;
            match raw {
                KeyCode::ArrowUp => history_prev(),
                KeyCode::ArrowDown => history_next(),
                _ => {} // Ignore other special keys
            }
        }
    }
}

fn push_char(c: char) {
    let mut buf_overflow = false;
    unsafe {
        if LINE_LEN < LINE_BUF_LEN - 1 {
            LINE_BUF[LINE_LEN] = c as u8;
            LINE_LEN += 1;
            print!("{}", c);
        } else {
            buf_overflow = true;
        }
    }
    if buf_overflow {
        println!("\n[buffer full]");
        unsafe {
            LINE_LEN = 0;
        }
        prompt();
    }
}

fn backspace() {
    unsafe {
        if LINE_LEN > 0 {
            LINE_LEN -= 1;
            crate::vga_buffer::backspace();
        }
    }
}

fn get_line() -> &'static str {
    unsafe {
        let slice = &LINE_BUF[..LINE_LEN];
        match str::from_utf8(slice) {
            Ok(s) => {
                LINE_LEN = 0;
                HISTORY_BROWSE_INDEX = None;
                s
            }
            Err(_) => {
                LINE_LEN = 0;
                HISTORY_BROWSE_INDEX = None;
                ""
            }
        }
    }
}

fn add_to_history(line: &str) {
    unsafe {
        if line.is_empty() {
            return;
        }

        // Copy to history
        let bytes = line.as_bytes();
        let len = bytes.len().min(LINE_BUF_LEN);
        HISTORY[HISTORY_INDEX][..len].copy_from_slice(&bytes[..len]);
        HISTORY_LENS[HISTORY_INDEX] = len;

        HISTORY_INDEX = (HISTORY_INDEX + 1) % HISTORY_SIZE;
        if HISTORY_COUNT < HISTORY_SIZE {
            HISTORY_COUNT += 1;
        }
    }
}

fn history_prev() {
    unsafe {
        if HISTORY_COUNT == 0 {
            return;
        }

        let browse_idx = match HISTORY_BROWSE_INDEX {
            None => {
                // Start browsing from most recent
                if HISTORY_COUNT < HISTORY_SIZE {
                    HISTORY_COUNT - 1
                } else {
                    (HISTORY_INDEX + HISTORY_SIZE - 1) % HISTORY_SIZE
                }
            }
            Some(idx) => {
                // Go to previous command
                if HISTORY_COUNT < HISTORY_SIZE {
                    if idx > 0 {
                        idx - 1
                    } else {
                        return; // At oldest command
                    }
                } else {
                    (idx + HISTORY_SIZE - 1) % HISTORY_SIZE
                }
            }
        };

        HISTORY_BROWSE_INDEX = Some(browse_idx);
        load_history_line(browse_idx);
    }
}

fn history_next() {
    unsafe {
        if let Some(idx) = HISTORY_BROWSE_INDEX {
            if HISTORY_COUNT < HISTORY_SIZE {
                if idx + 1 < HISTORY_COUNT {
                    let new_idx = idx + 1;
                    HISTORY_BROWSE_INDEX = Some(new_idx);
                    load_history_line(new_idx);
                } else {
                    // At newest, clear line
                    HISTORY_BROWSE_INDEX = None;
                    clear_current_line();
                }
            } else {
                let new_idx = (idx + 1) % HISTORY_SIZE;
                if new_idx != HISTORY_INDEX {
                    HISTORY_BROWSE_INDEX = Some(new_idx);
                    load_history_line(new_idx);
                } else {
                    HISTORY_BROWSE_INDEX = None;
                    clear_current_line();
                }
            }
        }
    }
}

fn load_history_line(idx: usize) {
    unsafe {
        // Clear current line
        clear_current_line();

        // Load history entry
        let len = HISTORY_LENS[idx];
        LINE_BUF[..len].copy_from_slice(&HISTORY[idx][..len]);
        LINE_LEN = len;

        // Display it
        if let Ok(s) = str::from_utf8(&LINE_BUF[..len]) {
            print!("{}", s);
        }
    }
}

fn clear_current_line() {
    unsafe {
        for _ in 0..LINE_LEN {
            crate::vga_buffer::backspace();
        }
        LINE_LEN = 0;
    }
}

fn execute_command(line: &str) {
    let parts = split_whitespace(line);
    if parts[0].is_empty() {
        return;
    }

    let cmd_name = parts[0];
    let args = &parts[1..];

    match find_command(cmd_name) {
        Some(cmd) => (cmd.func)(args),
        None => println!("Unknown command: {}. Type 'help' for available commands.", cmd_name),
    }
}

// ============================================================================
// Command implementations
// ============================================================================

fn cmd_help(_args: &[&str]) {
    println!("Available commands:");
    for cmd in COMMANDS {
        println!("  {:<12} - {}", cmd.name, cmd.help);
    }
}

fn cmd_echo(args: &[&str]) {
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            print!(" ");
        }
        print!("{}", arg);
    }
    println!("");
}

fn cmd_clear(_args: &[&str]) {
    crate::vga_buffer::clear_screen();
}

fn cmd_reboot(_args: &[&str]) {
    println!("Rebooting system...");
    crate::keyboard::reset_cpu();
}

fn cmd_history(_args: &[&str]) {
    unsafe {
        if HISTORY_COUNT == 0 {
            println!("No command history");
            return;
        }

        println!("Command history:");
        let start = if HISTORY_COUNT < HISTORY_SIZE {
            0
        } else {
            HISTORY_INDEX
        };

        for i in 0..HISTORY_COUNT {
            let idx = (start + i) % HISTORY_SIZE;
            let len = HISTORY_LENS[idx];
            if let Ok(s) = str::from_utf8(&HISTORY[idx][..len]) {
                println!("  {} {}", i + 1, s);
            }
        }
    }
}

// ============================================================================
// Utilities
// ============================================================================

/// Simple whitespace splitter that returns a fixed array of &str
fn split_whitespace(s: &str) -> [&str; 8] {
    let mut out: [&str; 8] = [""; 8];
    let mut idx = 0usize;
    let bytes = s.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() && idx < 8 {
        // Skip whitespace
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let start = i;
        while i < bytes.len() && bytes[i] != b' ' && bytes[i] != b'\t' {
            i += 1;
        }
        let token = &s[start..i];
        out[idx] = token;
        idx += 1;
    }

    out
}
