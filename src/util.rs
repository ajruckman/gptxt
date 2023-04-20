macro_rules! print_error {
    ($($arg:tt)*) => {{
        let mut stderr = std::io::stderr();
        queue!(stderr, crossterm::style::SetForegroundColor(crossterm::style::Color::Red)).unwrap();
        queue!(stderr, crossterm::style::SetAttribute(crossterm::style::Attribute::Bold)).unwrap();
        eprintln!($($arg)*);
        queue!(stderr, crossterm::style::ResetColor).unwrap();
        queue!(stderr, crossterm::style::SetAttribute(crossterm::style::Attribute::Reset)).unwrap();
    }};
}

macro_rules! print_success {
    ($($arg:tt)*) => {{
        let mut stderr = std::io::stderr();
        queue!(stderr, crossterm::style::SetForegroundColor(crossterm::style::Color::Green)).unwrap();
        queue!(stderr, crossterm::style::SetAttribute(crossterm::style::Attribute::Bold)).unwrap();
        eprintln!($($arg)*);
        queue!(stderr, crossterm::style::ResetColor).unwrap();
        queue!(stderr, crossterm::style::SetAttribute(crossterm::style::Attribute::Reset)).unwrap();
    }};
}

macro_rules! print_progress {
    ($($arg:tt)*) => {{
        let mut stderr = std::io::stderr();
        queue!(stderr, crossterm::style::SetForegroundColor(crossterm::style::Color::Cyan)).unwrap();
        queue!(stderr, crossterm::style::SetAttribute(crossterm::style::Attribute::Bold)).unwrap();
        eprintln!($($arg)*);
        queue!(stderr, crossterm::style::ResetColor).unwrap();
        queue!(stderr, crossterm::style::SetAttribute(crossterm::style::Attribute::Reset)).unwrap();
    }};
}
