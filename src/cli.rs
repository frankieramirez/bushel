use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

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
    /// Print a shell completion script to stdout
    Completions {
        /// Shell to generate completions for
        shell: CompletionShell,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
}

pub fn command() -> clap::Command {
    Args::command()
}

impl CompletionShell {
    pub const ALL: [Self; 3] = [Self::Bash, Self::Zsh, Self::Fish];

    pub fn artifact_name(self) -> &'static str {
        match self {
            Self::Bash => "bushel.bash",
            Self::Zsh => "bushel.zsh",
            Self::Fish => "bushel.fish",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_lists_the_completions_command() {
        let help = command().render_long_help().to_string();
        assert!(
            help.contains("completions"),
            "--help should list completions: {help}"
        );
    }

    #[test]
    fn completions_accepts_bash_zsh_fish() {
        for name in ["bash", "zsh", "fish"] {
            let args = Args::try_parse_from(["bushel", "completions", name])
                .unwrap_or_else(|e| panic!("completions {name} should parse: {e}"));
            assert!(matches!(args.command, Some(Cmd::Completions { .. })));
        }
    }

    #[test]
    fn completions_rejects_powershell() {
        assert!(Args::try_parse_from(["bushel", "completions", "powershell"]).is_err());
    }
}
