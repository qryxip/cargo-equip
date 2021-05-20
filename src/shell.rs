use std::{
    fmt,
    io::{self, Sink, Write},
};
use termcolor::{Color, ColorSpec, NoColor, StandardStream, WriteColor};

pub struct Shell {
    output: ShellOut,
}

impl Shell {
    pub fn new() -> Self {
        Self {
            output: ShellOut::stream(),
        }
    }

    pub fn from_stdout(stdout: Box<dyn Write>) -> Shell {
        Self {
            output: ShellOut::write(stdout),
        }
    }

    pub(crate) fn out(&mut self) -> &mut dyn Write {
        match &mut self.output {
            ShellOut::Stream { stdout, .. } => stdout,
            ShellOut::Write { stdout, .. } => stdout,
        }
    }

    pub fn err(&mut self) -> &mut dyn Write {
        match &mut self.output {
            ShellOut::Stream { stderr, .. } => stderr,
            ShellOut::Write { stderr, .. } => stderr,
        }
    }

    pub(crate) fn status(
        &mut self,
        status: impl fmt::Display,
        message: impl fmt::Display,
    ) -> io::Result<()> {
        self.print(status, message, Color::Green, true)
    }

    pub(crate) fn warn(&mut self, message: impl fmt::Display) -> io::Result<()> {
        self.print("warning", message, Color::Yellow, false)
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
        return match &mut self.output {
            ShellOut::Stream { stderr, .. } => print(stderr, status, message, color, justified),
            ShellOut::Write { .. } => {
                print(NoColor::new(io::sink()), status, message, color, justified)
            }
        };

        fn print(
            mut stderr: impl WriteColor,
            status: impl fmt::Display,
            message: impl fmt::Display,
            color: Color,
            justified: bool,
        ) -> io::Result<()> {
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
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

enum ShellOut {
    Stream {
        stdout: StandardStream,
        stderr: StandardStream,
    },
    Write {
        stdout: Box<dyn Write>,
        stderr: Sink,
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

    fn write(stdout: Box<dyn Write>) -> Self {
        Self::Write {
            stdout,
            stderr: io::sink(),
        }
    }
}
