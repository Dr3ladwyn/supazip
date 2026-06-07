use clap::CommandFactory;
use std::fs;
use std::path::Path;

pub fn generate_man_pages(out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(out_dir)?;
    let cmd = crate::Cli::command();
    let man = clap_mangen::Man::new(cmd);
    let mut buffer: Vec<u8> = Vec::new();
    man.render(&mut buffer)?;
    fs::write(out_dir.join("supazip.1"), buffer)?;
    Ok(())
}
