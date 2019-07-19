use log::warn;
use uefi::prelude::*;
use uefi::proto::media::file::{Directory, File, FileAttribute, FileHandle, FileMode, FileType};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::Completion;

use crate::util::to_string;

pub fn locate_file(
    boot_services: &BootServices,
    path: &[&str],
) -> uefi::Result<Option<FileHandle>> {
    let filesystem = boot_services.locate_protocol::<SimpleFileSystem>()?.log();
    // Loader is single-processor, so this should be the only reference to the protocol
    let filesystem = unsafe { &mut *filesystem.get() };

    locate_file_rec(filesystem.open_volume()?.log(), path)
}

fn locate_file_rec(mut dir: Directory, path: &[&str]) -> uefi::Result<Option<FileHandle>> {
    let mut buf = [0u8; 256];

    loop {
        match dir.read_entry(&mut buf).discard_errdata()?.log() {
            Some(entry) => {
                let name = to_string(entry.file_name());
                if name == path[0] {
                    let file = dir
                        .open(&name, FileMode::Read, FileAttribute::empty())?
                        .log();

                    if path.len() == 1 {
                        return Ok(Completion::from(Some(file)));
                    } else {
                        match file.into_type()?.log() {
                            FileType::Dir(next) => return locate_file_rec(next, &path[1..]),
                            FileType::Regular(_) => {
                                warn!("Expected {} to be a directory", name);
                                return Ok(Completion::from(None));
                            }
                        }
                    }
                }
            }
            None => return Ok(Completion::from(None)),
        }
    }
}
