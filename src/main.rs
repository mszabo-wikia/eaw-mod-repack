use clap::Parser;
use color_print::{cformat, cprintln};

use std::path::PathBuf;

mod lazy_file;
mod megfile_partitioner;
mod megfiles_xml;
mod mod_info;
mod packer;
mod steam;

static ABOUT_TEXT: &str = "Utility to create a local copy of an Empire at War mod with contents repacked into MEGA files.";
static LICENSE_NOTICE: &str = "Copyright 2026 the eaw-mod-repack contributors.

Licensed under the Apache License, Version 2.0 (the \"License\");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an \"AS IS\" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.";

#[derive(Parser, Debug)]
#[command(
    version,
    about = ABOUT_TEXT,
    long_about = cformat!("{}\n\nTypically one would run the tool providing either the Steam workshop ID or the source directory of the mod to repack, e.g.:\n
\t<y># repack Thrawn's Revenge</y>
\t<bold>eaw-mod-repack --steam-mod-id</bold> 1125571106
\t<y># repack some local mod</y>
\t<bold>eaw-mod-repack --source-dir</bold> path/to/some/mod\n
Optionally specify the Steam library root if you installed EaW in a non-default Steam library folder:\n
\t<y># repack Thrawn's Revenge in a custom Steam library</y>
\t<bold>eaw-mod-repack --steam-library-root path/to/my/SteamLibrary --steam-mod-id</bold> 1125571106\n
Using this for submods is probably redundant since submods tend to contain relatively few files.
Trying to pack submods may be possibly detrimental, since the tool has no knowledge of your mod load order
and therefore cannot determine which files should be exluded from packing.
This could cause submod overrides to not take effect.", ABOUT_TEXT),
    after_long_help = cformat!(
        "<bold>ACKNOWLEDGEMENTS</bold>\n\n{}\n<bold>COPYRIGHT</bold>\n\n{}",
        include_str!("../ACKNOWLEDGEMENTS.txt"),
        LICENSE_NOTICE
    )
)]
struct Args {
    /// Path to the mod to repack.
    #[arg(long)]
    source_dir: Option<PathBuf>,
    /// Path of the Empire at War installation directory.
    /// Useful if you have a non-Steam EaW install.
    #[arg(long)]
    eaw_root: Option<PathBuf>,
    /// Path to the Steam library folder under which Empire at War lives.
    /// Useful if you installed EaW in a non-default Steam folder.
    #[arg(long)]
    steam_library_root: Option<PathBuf>,
    /// Steam workshop ID of the mod to repack.
    #[arg(long)]
    steam_mod_id: Option<String>,
    /// Do not make any changes, just output what would happen.
    #[arg(long)]
    dry_run: bool,
    /// Enable verbose logging.
    #[arg(long)]
    debug: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    cprintln!("<bold>Empire at War Mod Repacker</bold>\n");

    let filter_level = if args.debug {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    env_logger::builder()
        .filter_level(filter_level)
        .format_target(false)
        .format_timestamp(None)
        .init();

    let dry_run = args.dry_run;

    if dry_run {
        log::info!("Dry-run mode: no changes will be made!");
    }

    let home_dir = std::env::home_dir();
    let eaw_root = steam::find_eaw_root(args.eaw_root, &args.steam_library_root, &home_dir)?;
    let source_dir = steam::find_source_mod_folder(
        args.steam_mod_id,
        &args.steam_library_root,
        &home_dir,
        args.source_dir,
    )?;

    let mod_name = mod_info::get_mod_name(&source_dir)?;

    let relative_dest_dir: PathBuf = ["Mods", &mod_name].iter().collect();
    let dest_dir = eaw_root.join("corruption").join(&relative_dest_dir);

    cprintln!(
        "\nWill create repacked version of mod <bold>\"{}\"</bold>",
        mod_name
    );
    cprintln!("Source folder: <bold>{}</bold>", &source_dir.display());

    if !dry_run {
        cprintln!(
            "Destination folder (will be removed): <bold>{}</bold>",
            &dest_dir.display()
        );
        println!("Proceed? [y/N]:");

        let mut input = String::new();

        std::io::stdin().read_line(&mut input)?;

        println!();

        if input.trim() != "y" {
            log::info!("Aborting.");
            return Ok(());
        }
    } else {
        cprintln!(
            "Destination folder (dry-run, will be left unchanged): <bold>{}</bold>\n",
            &dest_dir.display()
        );
    }

    packer::repack_mod(dry_run, mod_name, &eaw_root, &source_dir, &dest_dir)?;

    println!(
        "\nComplete! Run EAW with ModPath={}",
        &relative_dest_dir.display()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use crate::Args;

    #[test]
    fn verify_cli() {
        Args::command().debug_assert();
    }
}
