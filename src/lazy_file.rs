use std::{
    fs::File,
    io::{self, Read, Seek},
    path::PathBuf,
};

use petro_meg::writer::FileContent;

/// Helper struct to pack a given file into a MegBuilder
/// without needing to read the contents into memory
/// or keep the file open until the MEGA file is written.
#[derive(Debug)]
pub struct LazyFile {
    path: PathBuf,
    /// Lazy-initialized file handle.
    file: Option<File>,
}

impl LazyFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path, file: None }
    }
}

impl Read for LazyFile {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        if let Some(ref mut file) = self.file {
            file.read(buf)
        } else {
            let file = self.file.get_or_insert(File::open(&self.path)?);
            file.read(buf)
        }
    }
}

impl FileContent for LazyFile {
    fn file_len(&self) -> io::Result<u64> {
        Ok(std::fs::metadata(&self.path)?.len())
    }

    /// Ensure that the FileContent is at the correct start position in order to copy the number of
    /// bytes specified by file_len.
    fn ensure_at_start(&mut self) -> io::Result<()> {
        if let Some(ref mut file) = self.file {
            file.rewind()
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{self, BufReader, Read},
    };

    use petro_meg::writer::FileContent;
    use tempfile::TempDir;

    use crate::lazy_file::LazyFile;

    #[test]
    fn lazy_file_should_not_open_file_unless_read() -> io::Result<()> {
        let tmp_dir = TempDir::new().expect("could not create test dir");
        let path = tmp_dir.path().join("test.txt");
        fs::write(&path, "foo")?;

        let mut lazy_file = LazyFile::new(path);
        let rewind_res = lazy_file.ensure_at_start();
        let size = lazy_file.file_len().expect("size read should succeed");

        assert!(rewind_res.is_ok(), "rewind should succeed");
        assert_eq!(3, size, "size should match");
        assert!(
            lazy_file.file.is_none(),
            "should not open the file until read from"
        );

        Ok(())
    }

    #[test]
    fn lazy_file_reads_from_file() -> io::Result<()> {
        let tmp_dir = TempDir::new().expect("could not create test dir");
        let path = tmp_dir.path().join("test.txt");
        fs::write(&path, "foo")?;

        let mut lazy_file = LazyFile::new(path);
        let mut reader = BufReader::new(&mut lazy_file);
        let mut contents = String::new();
        reader.read_to_string(&mut contents)?;

        assert_eq!(contents, "foo", "content should match");
        assert!(lazy_file.file.is_some(), "should have opened the file");

        Ok(())
    }
}
