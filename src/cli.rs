//! bushel's command line, in the library rather than the binary so the docs
//! emitter can read the real flags — their names, their help text — instead of
//! keeping a second copy that would drift.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "bushel",
    version,
    about = "A lazydocker-style TUI for Apple Containers"
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Cmd>,
    /// Skip the splash screen
    #[arg(long)]
    pub no_splash: bool,
    /// Disable all animation and effects
    #[arg(long)]
    pub reduced_motion: bool,
    /// ASCII icons and spinners (no Unicode glyphs)
    #[arg(long)]
    pub ascii: bool,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Update bushel to the latest release
    Update,
}
