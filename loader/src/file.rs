//! Random-access I/O abstraction over the UEFI file protocols

use alloc::{vec, vec::Vec};

use plain::Plain;
use uefi::{
    prelude::*,
    proto::media::{
        file::{File as _, FileAttribute, FileMode, FileType, RegularFile},
        fs::SimpleFileSystem,
    },
};

pub struct File {
    inner: RegularFile
}

impl File {
    /// Opens the file at the given path on the root volume.
    ///
    /// # Panics
    /// If the file cannot be opened
    pub fn open(system_table: &SystemTable<Boot>, path: &str) -> File {
        let fs = system_table
            .boot_services()
            .locate_protocol::<SimpleFileSystem>()
            .expect_success("Could not locate SimpleFileSystem protocol");
        let mut root = unsafe {
            // Safety: within this UEFI application, this is the only use of SimpleFileSystem, so we have exclusive access
            let fs = &mut *fs.get();
            fs.open_volume()
                .expect_success("Could not open root directory")
        };

        let handle = root
            .open(path, FileMode::Read, FileAttribute::empty())
            .expect_success("Could not open file");

        match handle
            .into_type()
            .expect_success("Could not determine file type")
        {
            FileType::Regular(inner) => File { inner },
            _ => panic!("Not a file"),
        }
    }

    /// Reads as many bytes as possible into `buf`, starting at `offset` in the file.
    /// Returns the number of bytes read.
    pub fn read(&mut self, offset: usize, buf: &mut [u8]) -> uefi::Result<usize> {
        self.inner
            .set_position(offset as u64)
            .discard_errdata()
            .log_warning()?;

        self.inner
            .read(buf)
            .discard_errdata()
    }

    pub fn read_as<T: Plain + Default, const SIZE: usize>(&mut self, offset: usize) -> T {
        // The SIZE const generic is needed because core::mem::size_of can't be used with type parameters

        let mut buf = [0u8; SIZE];
        let bytes_read = self.read(offset, &mut buf).unwrap_success();
        if bytes_read != SIZE {
            panic!("Could not read {} bytes from file, got {}", SIZE, bytes_read);
        }

        let mut result = T::default();
        result.copy_from_bytes(&buf).expect("Buffer was too small");
        result
    }

    pub fn read_vec_as<T: Plain + Default + Clone, const SIZE: usize>(&mut self, offset: usize, count: usize) -> Vec<T> {
        let mut buf = vec![0u8; SIZE * count];
        let bytes_read = self.read(offset, &mut buf).unwrap_success();
        if bytes_read != SIZE * count {
            panic!("Could not read {} bytes from file ({} items of size {}), got {}", SIZE * count, count, SIZE, bytes_read);
        }

        let mut results = vec![T::default(); count];
        results.copy_from_bytes(&buf).expect("Buffer was too small");
        results
    }
}