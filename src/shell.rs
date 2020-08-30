use std::{
    fmt,
    io::{self, Write},
};
use termcolor::{Color, ColorSpec, NoColor, StandardStream, WriteColor as _};

pub struct Shell {
    output: ShellOut,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            output: ShellOut::stream(),
        }
    }

    pub fn err(&mut self) -> &mut dyn Write {
        let ShellOut::Stream { stderr, .. } = &mut self.output;
        stderr
    }

    pub(crate) fn status(
        &mut self,
        status: impl fmt::Display,
        message: impl fmt::Display,
    ) -> io::Result<()> {
        self.print(status, message, Color::Green, true)
    }

    pub fn error(&mut self, message: impl fmt::Display) -> io::Result<()> {
        self.print("error", message, Color::Red, false)
    }

    fn print(
        &mut self,
        status: impl fmt::Display,
        message: impl fmt::Display,
        color: Color,
        justified: bool,
    ) -> io::Result<()> {
        let ShellOut::Stream { stderr, .. } = &mut self.output;
        stderr.set_color(ColorSpec::new().set_bold(true).set_fg(Some(color)))?;
        if justified {
            write!(stderr, "{:>12}", status)?;
        } else {
            write!(stderr, "{}", status)?;
            stderr.set_color(ColorSpec::new().set_bold(true))?;
            write!(stderr, ":")?;
        }
        stderr.reset()?;
        writeln!(stderr, " {}", message)
    }
}

enum ShellOut {
    Stream {
        stdout: StandardStream,
        stderr: StandardStream,
    },
}

impl ShellOut {
    fn stream() -> Self {
        Self::Stream {
            stdout: StandardStream::stdout(if atty::is(atty::Stream::Stdout) {
                termcolor::ColorChoice::Auto
            } else {
                termcolor::ColorChoice::Never
            }),
            stderr: StandardStream::stderr(if atty::is(atty::Stream::Stderr) {
                termcolor::ColorChoice::Auto
            } else {
                termcolor::ColorChoice::Never
            }),
        }
    }
}
