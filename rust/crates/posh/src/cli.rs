use argh::{FromArgValue, FromArgs};

#[derive(FromArgs, Debug)]
#[argh(description = "")]
pub struct Cli {
    #[argh(subcommand)]
    pub command: Command,
}

#[derive(argh::FromArgs, Debug)]
#[argh(subcommand)]
pub enum Command {
    Center(Center),
    Pip(Pip),
}

#[derive(argh::FromArgs, Debug)]
#[argh(subcommand, name = "center", description = "Center a window.")]
pub struct Center {
    #[argh(positional, description = "window title")]
    pub title: String,
}

#[derive(argh::FromArgs, Debug)]
#[argh(
    subcommand,
    name = "pip",
    description = "Adjust PiP window position and size."
)]
pub struct Pip {
    #[argh(option, description = "horizontal position (left/right)")]
    pub horizontal: HorizontalPosition,
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum HorizontalPosition {
    Left,
    Right,
}

impl FromArgValue for HorizontalPosition {
    fn from_arg_value(value: &str) -> Result<Self, String> {
        match value {
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            _ => Err("unexpected argument, expecting `left` or `right`".to_owned()),
        }
    }
}
