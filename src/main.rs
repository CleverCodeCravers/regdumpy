use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use regdumpy::dumper::dump_registry;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Output file path where the registry dump will be written
    #[arg(short, long, value_name = "FILE")]
    output: PathBuf,

    /// Root registry hive to start the dump from (e.g. HKEY_LOCAL_MACHINE)
    #[arg(short, long, value_name = "ROOT", default_value = "HKEY_CURRENT_USER")]
    root: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if !is_elevated::is_elevated() {
        eprintln!("WARNING: You are not running as Administrator. Some registry data may be inaccessible.");
    }

    dump_registry(&cli.output, &cli.root)?;
    println!("Registry successfully written to {}", cli.output.display());
    Ok(())
}
