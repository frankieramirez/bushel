fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--out-dir") => {
            let Some(dir) = args.next() else {
                eprintln!("dist-assets: --out-dir needs a path");
                std::process::exit(2);
            };
            for name in bushel::completions::write_dist_assets(dir.as_ref())? {
                eprintln!("dist-assets: wrote {dir}/{name}");
            }
        }
        Some(other) => {
            eprintln!("dist-assets: unknown argument `{other}` (expected --out-dir <path>)");
            std::process::exit(2);
        }
        None => {
            eprintln!("dist-assets: expected --out-dir <path>");
            std::process::exit(2);
        }
    }
    Ok(())
}
