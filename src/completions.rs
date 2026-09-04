use std::path::Path;

use clap_complete::{Shell, generate};
use clap_mangen::Man;

use crate::cli::CompletionShell;

impl From<CompletionShell> for Shell {
    fn from(shell: CompletionShell) -> Self {
        match shell {
            CompletionShell::Bash => Self::Bash,
            CompletionShell::Zsh => Self::Zsh,
            CompletionShell::Fish => Self::Fish,
        }
    }
}

pub fn write_script(shell: CompletionShell, out: &mut dyn std::io::Write) -> std::io::Result<()> {
    let mut cmd = crate::cli::command();
    let mut buf = Vec::new();
    generate(Shell::from(shell), &mut cmd, "bushel", &mut buf);
    out.write_all(&buf)
}

pub fn write_man(out: &mut dyn std::io::Write) -> std::io::Result<()> {
    Man::new(crate::cli::command()).render(out)
}

pub fn man_filename() -> String {
    Man::new(crate::cli::command()).get_filename()
}

pub fn write_dist_assets(dir: &Path) -> std::io::Result<Vec<String>> {
    std::fs::create_dir_all(dir)?;
    let mut wrote = Vec::new();
    for shell in CompletionShell::ALL {
        let name = shell.artifact_name();
        let path = dir.join(name);
        let mut file = std::fs::File::create(&path)?;
        write_script(shell, &mut file)?;
        wrote.push(name.to_string());
    }
    let man_name = man_filename();
    let mut file = std::fs::File::create(dir.join(&man_name))?;
    write_man(&mut file)?;
    wrote.push(man_name);
    Ok(wrote)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(shell: CompletionShell) -> String {
        let mut buf = Vec::new();
        write_script(shell, &mut buf).expect("completion script writes");
        String::from_utf8(buf).expect("completion script is utf-8")
    }

    #[test]
    fn each_shell_emits_a_nonempty_script_from_clap() {
        for shell in CompletionShell::ALL {
            let text = script(shell);
            assert!(!text.trim().is_empty(), "{shell:?} script is empty");
            assert!(
                text.to_ascii_lowercase().contains("bushel"),
                "{shell:?} script should name the binary: {text}"
            );
            assert!(
                text.contains("completions") || text.contains("update"),
                "{shell:?} script should mention a clap subcommand: {text}"
            );
        }
    }

    #[test]
    fn man_page_is_bushel_1_from_clap() {
        assert_eq!(man_filename(), "bushel.1");
        let mut buf = Vec::new();
        write_man(&mut buf).expect("man page writes");
        let text = String::from_utf8(buf).expect("man page is utf-8");
        assert!(!text.trim().is_empty(), "man page is empty");
        assert!(
            text.to_ascii_uppercase().contains("BUSHEL"),
            "man page should title bushel: {text}"
        );
        assert!(
            text.contains("completions"),
            "man page should document the completions command: {text}"
        );
    }

    #[test]
    fn dist_assets_write_the_release_filenames() {
        let dir = std::env::temp_dir().join(format!("bushel-dist-assets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let wrote = write_dist_assets(&dir).expect("dist assets write");
        assert_eq!(
            wrote,
            vec!["bushel.bash", "bushel.zsh", "bushel.fish", "bushel.1"]
        );
        for name in &wrote {
            let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!bytes.is_empty(), "{name} is empty");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
