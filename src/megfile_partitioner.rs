use std::{
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::Context;
use petro_meg::{
    path::MegPath,
    version::MegV1,
    writer::{BuildMeg, FileContent, MegBuilder, WriteVersion},
};

/// Pack files into MEGA files following a given name pattern,
/// adding new MEGA files when size limits are exhausted.
pub struct MegFilePartitioner<F: FileContent> {
    builders: Vec<MegBuilder<F, MegV1>>,
    base_name: String,
    cur_size: u32,
}

impl<F: FileContent> MegFilePartitioner<F> {
    pub fn new(base_name: String) -> Self {
        MegFilePartitioner {
            builders: vec![],
            base_name,
            cur_size: MegV1.header_size(),
        }
    }

    /// Add the file under the given path into this MEGA file.
    pub fn insert(&mut self, root: &Path, path: &Path, contents: F) -> anyhow::Result<()> {
        let relative_path = path
            .strip_prefix(root)?
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("invalid filename {}", path.display()))?;

        let mut meg_path = MegPath::from_str(relative_path)
            .with_context(|| format!("Error creating MEGA file path for {}", path.display()))?
            .to_owned();
        meg_path.make_normalized();

        log::debug!(
            "Inserting {} under {} into {}",
            &path.display(),
            meg_path.as_str(),
            self.cur_builder_name()
        );

        let name_record_len = (size_of::<u16>() + meg_path.len()) as u32;
        let data_len = contents.file_len()? as u32;

        // Although the theoretical maximum file size of a MEGA file is 4GiB,
        // EAW only supports 2GiB MEGA files.
        static MAX_SIZE: u32 = i32::MAX as u32;

        let added_len = data_len + name_record_len + MegV1.file_record_size();

        if added_len + MegV1.header_size() > MAX_SIZE {
            anyhow::bail!(
                "{} is too large to fit into an EaW MEGA file",
                path.display()
            );
        }

        // Add to the current MEGA file if we fit into size limits, else start a new one.
        if !self.builders.is_empty()
            && self
                .cur_size
                .checked_add(added_len)
                .is_some_and(|n| n < MAX_SIZE)
        {
            self.cur_size += added_len;
        } else {
            self.cur_size = MegV1.header_size() + added_len;
            self.builders.push(MegV1::builder(MegV1));
        }

        let builder = self.builders.last_mut().unwrap();
        builder.insert(meg_path, contents);

        Ok(())
    }

    /// Write all MEGA files to disk.
    pub fn build(&mut self, dry_run: bool, dest_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
        let mut output_names = Vec::with_capacity(self.builders.len());
        while let Some(builder) = self.builders.pop() {
            let meg_name = format!("{}_{}.meg", self.base_name, self.builders.len() + 1);

            let (file_count, total_size) = builder
                .files()
                .try_fold(
                    (0, 0),
                    |(file_count, total_size), entry| -> anyhow::Result<(u32, u64)> {
                        Ok((file_count + 1, total_size + entry.file_len()?))
                    },
                )
                .with_context(|| format!("Error calculating MEGA file stats for {}", meg_name))?;

            let (formatted_size, unit) = if total_size > 1024_u64.pow(3) {
                (total_size as f64 / 1024_f64.powf(3.0), "GiB")
            } else {
                (total_size as f64 / 1024_f64.powf(2.0), "MiB")
            };

            log::info!(
                "Creating {} ({} files, {:.2} {})",
                meg_name,
                file_count,
                formatted_size,
                unit
            );

            let output_path = dest_dir.join(&meg_name);

            if !dry_run {
                let mut output = File::create(&output_path)?;
                builder.build(&mut output)?;
            }

            output_names.push(output_path)
        }

        Ok(output_names)
    }

    /// Get the name of the MEGA file currently being populated.
    pub fn cur_builder_name(&self) -> String {
        format!("{}-{}.meg", self.base_name, self.builders.len())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::Read,
        path::{Path, PathBuf},
    };

    use petro_meg::{
        path::MegPath,
        version::MegV1,
        writer::{FileContent, WriteVersion},
    };

    use crate::megfile_partitioner::MegFilePartitioner;

    #[derive(Debug, Clone)]
    struct MockFile {
        size: u64,
        path: PathBuf,
    }

    impl MockFile {
        fn new(size: u64, path: PathBuf) -> Self {
            Self { size, path }
        }
    }

    impl PartialEq for MockFile {
        fn eq(&self, other: &Self) -> bool {
            self.size == other.size && self.path == other.path
        }
    }

    impl Read for MockFile {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            todo!()
        }
    }

    impl FileContent for MockFile {
        fn file_len(&self) -> std::io::Result<u64> {
            Ok(self.size)
        }

        fn ensure_at_start(&mut self) -> std::io::Result<()> {
            todo!()
        }
    }

    #[test]
    fn rejects_overly_large_files() {
        let mut builder = MegFilePartitioner::new("test".into());
        let test_file_path = Path::new("/test/mod/Data/Art/big.alo");

        let cases = [
            // over 2GiB
            MockFile::new(i32::MAX as u64 + 256, test_file_path.to_path_buf()),
            // would be over 2GiB when considering metadata size
            MockFile::new(i32::MAX as u64 - 32, test_file_path.to_path_buf()),
        ];

        for file in cases {
            let err = builder
                .insert(Path::new("/test/mod"), test_file_path, file)
                .expect_err("should fail");

            assert_eq!(
                "/test/mod/Data/Art/big.alo is too large to fit into an EaW MEGA file",
                err.to_string()
            );
            assert_eq!(
                MegV1.header_size(),
                builder.cur_size,
                "size should be the same"
            );
            assert!(
                builder.builders.is_empty(),
                "should not create new MEGA file builder"
            );
        }
    }

    #[test]
    fn should_insert_valid_file() {
        let mut partitioner = MegFilePartitioner::new("test".into());
        let test_file_path = Path::new("/test/mod/Data/Art/big.alo");
        let size = 16_000_000;
        let file = MockFile::new(size, test_file_path.to_path_buf());

        partitioner
            .insert(
                Path::new("/test/mod"),
                Path::new("/test/mod/Data/Art/big.alo"),
                file,
            )
            .expect("should not fail to insert");

        assert_eq!(
            MegV1.header_size() + MegV1.file_record_size() + 18 + 16_000_000,
            partitioner.cur_size,
            "should update the size with the new name record, file record and data length"
        );
        assert_eq!(1, partitioner.builders.len());

        let cur_meg = partitioner.builders.first().unwrap();
        assert_eq!(1, cur_meg.paths().len(), "should insert the file");
        assert_eq!(
            Some(MegPath::from_str("DATA\\ART\\BIG.ALO").unwrap()),
            cur_meg.paths().next(),
            "should insert the file under a normalized relative path"
        );
        assert_eq!(1, cur_meg.files().len(), "should insert the file contents");
        assert_eq!(
            Some(&MockFile {
                size,
                path: test_file_path.to_path_buf()
            }),
            cur_meg.files().next(),
            "should insert the file contents as given"
        );
    }

    #[test]
    fn should_split_files_between_multiple_megs_if_needed() {
        let size = 32_000_000;
        let mut partitioner = MegFilePartitioner::new("test".into());

        for i in 1..193 {
            let test_file_name = format!("/test/mod/Data/Art/big{}.alo", i);
            let test_file_path = Path::new(&test_file_name);
            let file = MockFile::new(size, test_file_path.to_path_buf());

            partitioner
                .insert(Path::new("/test/mod"), test_file_path, file)
                .expect("should not fail to insert");
        }

        assert_eq!(
            3,
            partitioner.builders.len(),
            "should split the files between 3 MEGA files"
        );

        const EXPECTED_FILES_PER_BUILDER: usize = 67;
        let mut total_file_count = 0;
        for builder in partitioner.builders {
            let paths = builder.paths().collect::<Vec<_>>();

            assert!(
                paths.len() <= EXPECTED_FILES_PER_BUILDER,
                "should put no more than {} files per builder",
                EXPECTED_FILES_PER_BUILDER
            );
            total_file_count += paths.len();
        }

        assert_eq!(
            192, total_file_count,
            "should put every file into a builder"
        );
    }
}
