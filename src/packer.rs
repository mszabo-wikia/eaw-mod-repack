use std::{
    collections::HashSet,
    ffi::OsStr,
    fs::{self, File},
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::Context;

use petro_meg::{
    path::{MegPath, MegPathBuf},
    reader::ReadMegMeta,
    version::MegV1,
};

use crate::{lazy_file::LazyFile, megfile_partitioner::MegFilePartitioner, megfiles_xml};

/// Determine whether a given file should be packed into a MEGA file.
fn should_pack_file(source_dir: &Path, core_game_paths: &HashSet<MegPathBuf>, path: &Path) -> bool {
    // NB.: intentionally excluding scripts and maps since both TR and FoTR have them in a MEGA file already.
    static FILE_TYPES_TO_PACK: [&str; 8] = ["dds", "tga", "alo", "ala", "xml", "wav", "mp3", "png"];

    if let Some(file_type) = path.extension()
        && let Ok(relative_path) = path.strip_prefix(source_dir)
    {
        // Some stray XML files live outside Data/XML, let's not bother creating useless MEGA files for them.
        if file_type.eq_ignore_ascii_case("xml")
            && !relative_path
                .iter()
                .take(2)
                .collect::<PathBuf>()
                .as_os_str()
                .eq_ignore_ascii_case("Data/XML")
        {
            return false;
        }

        if FILE_TYPES_TO_PACK
            .iter()
            .map(&OsStr::new)
            .any(|s| s.eq_ignore_ascii_case(file_type))
        {
            // Only pack up files with allowed extensions that don't match the path of a base game file.
            let result = relative_path
                .to_str()
                .map(&str::to_string)
                .as_mut()
                .and_then(|s| MegPath::from_str_mut(s).ok())
                .is_some_and(|p| {
                    p.make_normalized();
                    !core_game_paths.contains(p)
                });

            if !result {
                log::debug!(
                    "Excluding {} from packing because it matches a core game path",
                    relative_path.display()
                );
            }

            return result;
        }
    }

    false
}

/// Copy a file from A to B with debug logging and other ceremony.
fn copy_file(
    dry_run: bool,
    source: &Path,
    file_name: &OsStr,
    dest_dir: &Path,
) -> anyhow::Result<u64> {
    let dest = dest_dir.join(file_name);
    log::debug!("Copying {} to {}", source.display(), &dest.display());
    if !dry_run {
        std::fs::create_dir_all(dest_dir)
            .with_context(|| format!("Failed to create {}", &dest_dir.display()))?;
        std::fs::copy(source, &dest)
            .with_context(|| format!("Failed to copy {} to {}", source.display(), dest.display()))
    } else {
        Ok(0)
    }
}

/// Get the normalized relative paths of files included in FoC's base MEGA files.
fn get_core_game_paths(eaw_dir: &Path) -> anyhow::Result<HashSet<MegPathBuf>> {
    let mut core_paths = HashSet::new();

    log::info!("Discovering base game files");

    for entry in fs::read_dir(eaw_dir.join("corruption/Data"))
        .context("Error while reading base game files")?
    {
        let path = entry.context("Error while reading base game files")?.path();

        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("meg"))
        {
            log::debug!("Loading base game paths from {}", path.display());

            let meg_file = File::open(&path)?;
            for meg_entry in MegV1
                .read_meg_meta(meg_file)
                .with_context(|| format!("Error while reading {}", path.display()))?
            {
                let mut meg_path = meg_entry.name().to_owned();
                // should already be, but just in case
                meg_path.make_normalized();
                core_paths.insert(meg_path);
            }
        }
    }

    log::info!(
        "Excluded {} base game paths from repacking",
        core_paths.len()
    );

    Ok(core_paths)
}

/// Recursively package a directory, adding discovered files to MEGA files if valid for inclusion
/// and copying them to the destination directory if not.
/// Returns the number of files processed.
fn package_files(
    root: &Path,
    source_dir: &Path,
    dest_dir: &Path,
    core_game_paths: &HashSet<MegPathBuf>,
    builder: &mut MegFilePartitioner<LazyFile>,
    dry_run: bool,
) -> anyhow::Result<u32> {
    let mut num_files_processed = 0;
    if source_dir.is_dir() {
        for entry in fs::read_dir(source_dir)
            .with_context(|| format!("Error while reading {}", source_dir.display()))?
        {
            let entry =
                entry.with_context(|| format!("Error while reading {}", source_dir.display()))?;
            let path = entry.path();
            let file_name = path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid filename {}", path.display()))?;

            if path.is_dir() {
                let cur_dest_dir = dest_dir.join(file_name);
                num_files_processed += package_files(
                    root,
                    &path,
                    &cur_dest_dir,
                    core_game_paths,
                    builder,
                    dry_run,
                )?;
            } else if should_pack_file(root, core_game_paths, &path) {
                let contents = LazyFile::new(path.clone());
                builder.insert(root, &path, contents).with_context(|| {
                    format!(
                        "Failed to insert {} into {}",
                        &path.display(),
                        builder.cur_builder_name()
                    )
                })?;
                num_files_processed += 1;
            } else {
                copy_file(dry_run, &path, file_name, dest_dir)?;
                num_files_processed += 1;
            }
        }
    }

    Ok(num_files_processed)
}

/// Create a local copy of the given source mod, with eligible files packed into MEGA files.
pub fn repack_mod(
    dry_run: bool,
    mod_name: String,
    eaw_dir: &Path,
    source_dir: &Path,
    dest_dir: &Path,
) -> anyhow::Result<()> {
    if !dry_run {
        let res = std::fs::remove_dir_all(dest_dir);
        if let Err(ref e) = res
            && e.kind() != ErrorKind::NotFound
        {
            return res.context("Failed to remove destination folder");
        }
    }

    let dest_data_dir = dest_dir.join("Data");

    if !dry_run {
        std::fs::create_dir_all(&dest_data_dir).context("Failed to create destination folder")?;
    }

    let source_data_dir = source_dir.join("Data");

    let megfiles_xml_path = source_data_dir.join("megafiles.xml");
    let mut mega_entries = if megfiles_xml_path.is_file() {
        let file = File::open(&megfiles_xml_path).context("Failed to open megafiles.xml")?;
        megfiles_xml::get_entries(file).context("Error reading megafiles.xml")?
    } else {
        vec![]
    };

    log::info!(
        "Loaded {} existing entries from {}",
        mega_entries.len(),
        &megfiles_xml_path.display()
    );

    log::info!("Processing files...");

    let mut num_files_processed = 0;

    for entry in fs::read_dir(source_dir)
        .with_context(|| format!("Error while reading {}", source_dir.display()))?
    {
        let path = entry
            .with_context(|| format!("Error while reading {}", source_dir.display()))?
            .path();

        if path.is_file() {
            let file_name = path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid filename {}", path.display()))?;

            copy_file(dry_run, &path, file_name, dest_dir)?;
            num_files_processed += 1;
        }
    }

    // Find the relative paths of base game files, so that we don't pack up modded files with matching paths.
    // petrolution.net says MEG files loaded later should be merged by the game but evidently this isn't working as described.
    let core_game_paths = get_core_game_paths(eaw_dir)?;

    let mut meg_builders = vec![];

    for entry in fs::read_dir(&source_data_dir)
        .with_context(|| format!("Error while reading {}", source_data_dir.display()))?
    {
        let path = entry
            .with_context(|| format!("Error while reading {}", source_data_dir.display()))?
            .path();
        let file_name = path
            .file_name()
            .and_then(&OsStr::to_str)
            .ok_or_else(|| anyhow::anyhow!("Invalid filename {}", path.display()))?;

        if path.is_dir() {
            let mut builder = MegFilePartitioner::new(format!("{}_{}", mod_name, file_name));

            num_files_processed += package_files(
                source_dir,
                &path,
                &dest_data_dir.join(file_name),
                &core_game_paths,
                &mut builder,
                dry_run,
            )?;

            meg_builders.push(builder);
        } else {
            copy_file(dry_run, &path, OsStr::new(file_name), &dest_data_dir)?;
            num_files_processed += 1;
        }
    }

    log::info!("Processed {} files", num_files_processed);

    for mut builder in meg_builders {
        let output_paths = builder.build(dry_run, &dest_data_dir)?;

        for output_path in output_paths {
            let relative_output_path = output_path
                .strip_prefix(dest_dir)
                .unwrap()
                .to_str()
                .unwrap()
                .replace("/", "\\");

            mega_entries.push(relative_output_path);
        }
    }

    let dest_megafiles_xml = dest_data_dir.join("megafiles.xml");

    log::info!(
        "Writing {} entries to {}",
        mega_entries.len(),
        &dest_megafiles_xml.display()
    );

    if !dry_run {
        let mut file =
            File::create(&dest_megafiles_xml).context("Error while creating megafiles.xml")?;
        megfiles_xml::write_entries(&mut file, &mega_entries)
            .context("Error writing megafiles.xml")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        fs::{self, File},
        io::Cursor,
        path::Path,
    };

    use petro_meg::{
        path::MegPathBuf,
        reader::{FileEntry, ReadMegMeta},
        version::MegV1,
        writer::BuildMeg,
    };
    use tempfile::TempDir;

    use crate::{
        megfiles_xml,
        packer::{repack_mod, should_pack_file},
    };

    #[test]
    fn should_only_pack_matching_files() {
        let source_root = Path::new("/some/eaw");

        let core_game_files: HashSet<MegPathBuf> = [
            "Data/Xml/Other.xml",
            "Data/Art/Textures/Menuback_Overlay.dds",
        ]
        .into_iter()
        .map(&str::to_string)
        .map(|s| MegPathBuf::from_string(s).unwrap())
        .map(|mut p| {
            p.make_normalized();
            p
        })
        .collect();

        let valid_cases = [
            "Data/XML/Props/Foo.xml",
            "DATA/XML/AI/IMPORTANT.XML",
            "Data/Art/Models/WALKER.ALA",
            "DATA/ART/MODELS/FOO.ALO",
            "Data/Art/Textures/foo.dds",
        ];

        let invalid_cases = [
            "Data/XML/.gitignore",
            "Data/XML/foo.txt",
            "Data/Xml/Other.xml",
            "Data/Text/MasterTextFile.xml",
            "DATA/TEXT/MASTERTEXTFILE.XML",
            "Data/Art/Textures/Menuback_Overlay.dds",
            "DATA/ART/TEXTURES/MENUBACK_OVERLAY.DDS",
        ];

        for case in valid_cases {
            assert!(
                should_pack_file(source_root, &core_game_files, &source_root.join(case)),
                "{} should be packed",
                case
            )
        }

        for case in invalid_cases {
            assert!(
                !should_pack_file(source_root, &core_game_files, &source_root.join(case)),
                "{} should not be packed",
                case
            )
        }
    }

    fn create_test_mod(source_dir: &Path) -> anyhow::Result<()> {
        fs::create_dir_all(source_dir.join("Data/Art/Textures"))?;
        fs::create_dir_all(source_dir.join("Data/XML/Foo"))?;
        fs::create_dir_all(source_dir.join("Data/Scripts"))?;
        fs::write(source_dir.join("Data/Art/Textures/foo.dds"), "aaa")?;
        fs::write(source_dir.join("Data/Art/Textures/Excluded.dds"), "aaa")?;
        fs::write(
            source_dir.join("Data/XML/Other.xml"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
        )?;
        fs::write(
            source_dir.join("Data/XML/Foo/Test.xml"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
        )?;
        fs::write(source_dir.join("Foo.txt"), "bla")?;
        fs::write(source_dir.join("Data/Scripts/Bar.lua"), "ble")?;

        Ok(())
    }

    #[test]
    fn repack_mod_does_not_do_anything_if_dry_run() -> anyhow::Result<()> {
        let tmp = TempDir::new().expect("failed to create test dir");
        let eaw_dir = tmp.path().join("TestEaw");
        let foc_data_dir = eaw_dir.join("corruption/Data");
        let dest_dir = eaw_dir.join("corruption/Mods/TestDest");
        let source_dir = tmp.path().join("TestSource");

        fs::create_dir_all(&dest_dir)?;
        fs::create_dir_all(&foc_data_dir)?;

        create_test_mod(&source_dir)?;

        repack_mod(true, "TestMod".into(), &eaw_dir, &source_dir, &dest_dir)?;

        assert!(
            dest_dir.is_dir(),
            "destination directory should still exist"
        );
        assert_eq!(
            fs::read_dir(&dest_dir)?.count(),
            0,
            "destination directory should be empty"
        );

        Ok(())
    }

    #[test]
    fn repack_mod_creates_repacked_local_copy() -> anyhow::Result<()> {
        run_repack_test(vec![])
    }

    #[test]
    fn repack_mod_creates_repacked_local_copy_with_original_megs_preserved() -> anyhow::Result<()> {
        run_repack_test(vec!["Data\\EnglishSpeech.meg".to_string()])
    }

    /// Convenience function to create a test MEGA file holding a single entry with mock data.
    fn create_meg(output_path: &Path, entry_path: &str) -> anyhow::Result<()> {
        let mock_data = Cursor::new("aaaa".as_bytes());
        let mut meg = MegV1::builder(MegV1);
        let mut entry_meg_path = MegPathBuf::from_string(entry_path.to_string())?;
        entry_meg_path.make_normalized();
        meg.insert(entry_meg_path, mock_data);

        let mut output_file = File::create(output_path)?;
        meg.build(&mut output_file)?;

        Ok(())
    }

    /// Convenience function to list the files contained by a MEGA file.
    fn get_meg_entries(meg_path: &Path) -> anyhow::Result<Vec<String>> {
        let meg = File::open(meg_path)?;
        let meg_meta = MegV1.read_meg_meta(meg)?;

        Ok(meg_meta
            .iter()
            .map(&FileEntry::name)
            .map(|p| p.to_string())
            .collect::<Vec<_>>())
    }

    fn run_repack_test(original_mega_files: Vec<String>) -> anyhow::Result<()> {
        let tmp = TempDir::new().expect("failed to create test dir");
        let eaw_dir = tmp.path().join("TestEaw");
        let foc_data_dir = eaw_dir.join("corruption/Data");
        let dest_dir = eaw_dir.join("corruption/Mods/TestDest");
        let source_dir = tmp.path().join("TestSource");

        fs::create_dir_all(&dest_dir)?;
        fs::create_dir_all(&foc_data_dir)?;

        // Create two MEGA files emulating base game files.
        // Files with identical relative paths should not be packed up.
        create_meg(&foc_data_dir.join("config.meg"), "Data/Xml/Other.xml")?;
        create_meg(
            &foc_data_dir.join("ART.MEG"),
            "Data/Art/Textures/Excluded.dds",
        )?;

        create_test_mod(&source_dir)?;

        if !original_mega_files.is_empty() {
            let mut source_megfiles = File::create(source_dir.join("Data/megafiles.xml"))?;
            megfiles_xml::write_entries(&mut source_megfiles, &original_mega_files)?;
        }

        repack_mod(false, "TestMod".into(), &eaw_dir, &source_dir, &dest_dir)?;

        let mut mega_files =
            megfiles_xml::get_entries(File::open(dest_dir.join("Data/megafiles.xml"))?)?;

        let mut expected_mega_files = original_mega_files
            .into_iter()
            .chain([
                "Data\\TestMod_XML_1.meg".to_string(),
                "Data\\TestMod_Art_1.meg".to_string(),
            ])
            .collect::<Vec<_>>();

        mega_files.sort();
        expected_mega_files.sort();

        assert_eq!(
            expected_mega_files, mega_files,
            "should have written the expected MEGA files to megafiles.xml"
        );

        assert_eq!(
            get_meg_entries(&dest_dir.join("Data/TestMod_Art_1.meg"))?,
            vec!["DATA\\ART\\TEXTURES\\FOO.DDS"],
            "DDS MEG should hold the DDS file under its canonicalized path"
        );

        assert_eq!(
            get_meg_entries(&dest_dir.join("Data/TestMod_XML_1.meg"))?,
            vec!["DATA\\XML\\FOO\\TEST.XML"],
            "XML MEG should hold the XML file under its canonicalized path"
        );

        assert!(
            !dest_dir.join("Data/Art/Textures/foo.dds").is_file(),
            "should not have copied over the DDS file"
        );

        assert!(
            !dest_dir.join("Data/XML/Test.xml").is_file(),
            "should not have copied over the XML file"
        );

        assert!(
            dest_dir.join("Data/Scripts/Bar.lua").is_file(),
            "should have copied over the Lua script"
        );

        assert!(
            dest_dir.join("Data/Art/Textures/Excluded.dds").is_file(),
            "should have copied over the other DDS file"
        );

        assert!(
            dest_dir.join("Data/XML/Other.xml").is_file(),
            "should have copied over XML file directly under Data/XML"
        );

        assert!(
            dest_dir.join("Foo.txt").is_file(),
            "should have copied over the file in the mod root"
        );

        Ok(())
    }
}
