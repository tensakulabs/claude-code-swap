use std::io::IsTerminal;

pub fn is_stdout_tty() -> bool {
    std::io::stdout().is_terminal()
}

pub fn is_stderr_tty() -> bool {
    std::io::stderr().is_terminal()
}

pub fn err_msg(msg: &str) {
    if is_stderr_tty() {
        eprintln!("\x1b[31mccs: error:\x1b[0m {msg}");
    } else {
        eprintln!("ccs: error: {msg}");
    }
}

pub fn warn_msg(msg: &str) {
    if is_stderr_tty() {
        eprintln!("\x1b[33mccs: warning:\x1b[0m {msg}");
    } else {
        eprintln!("ccs: warning: {msg}");
    }
}

pub fn green(s: &str) -> String {
    if is_stdout_tty() {
        format!("\x1b[32m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn yellow(s: &str) -> String {
    if is_stdout_tty() {
        format!("\x1b[33m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn red(s: &str) -> String {
    if is_stdout_tty() {
        format!("\x1b[31m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}
