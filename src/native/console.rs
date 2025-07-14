#![allow(unreachable_code)]

#[cfg(target_family = "windows")]
use crate::native::win32::WinRawConsole;

pub enum ConsoleError {
    RawInputMode,
    NoSTDINHandle,
}

pub enum ConsoleReadError {
    FailedRead,
    InvalidEventType,
}

#[derive(Debug)]
#[allow(unused)]
pub struct ReadEvent {
    pub virtual_key: u16,
    pub character: Option<char>,
    pub ralt_pressed: bool,
    pub lalt_pressed: bool,
    pub shift_pressed: bool,
    pub ctrl_pressed: bool,
}

pub struct Console {
    #[cfg(target_family = "windows")]
    inner: WinRawConsole,
}

impl Console {
    pub fn new() -> Result<Self, ConsoleError> {
        #[cfg(target_family = "windows")]
        {
            let wrcon = match WinRawConsole::new() {
                Ok(w) => w,
                Err(e) => {
                    use crate::native::win32::RawCreateError;

                    println!("Could not create raw console.");
                    let err = match e {
                        RawCreateError::InputErr => {
                            println!("Could not set raw input mode.");
                            ConsoleError::RawInputMode
                        }
                        RawCreateError::CreateErr(_) => {
                            println!("Could not obtain STDIN handle.");
                            ConsoleError::NoSTDINHandle
                        }
                    };
                    println!(
                        "Special control sequences such as CTRL + C may not work/have unintended behavior!"
                    );

                    return Err(err);
                }
            };

            return Ok(Self { inner: wrcon });
        }

        todo!()
    }

    pub fn read(&mut self) -> Result<ReadEvent, ConsoleReadError> {
        #[cfg(target_family = "windows")]
        {
            use crate::native::win32::ReadError;

            return match self.inner.read() {
                Ok(e) => Ok(e),
                Err(e) => match e {
                    ReadError::FailedConsoleRead => Err(ConsoleReadError::FailedRead),
                    ReadError::InvalidEventType => Err(ConsoleReadError::InvalidEventType),
                },
            };
        }

        todo!()
    }
}
