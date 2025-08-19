use argh::FromArgs;

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
pub struct Pip {}
