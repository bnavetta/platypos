//! Platform-specific QEMU setup code for x86-64

use crate::prelude::*;

pub fn build_boot_image(binary: &Utf8Path) -> Result<Utf8PathBuf> {
    // To get to the target directory, go up two levels (kernel binary is in
    // `target/$mode/$target/`)
    let target_dir = binary
        .parent()
        .and_then(|p| p.parent())
        .ok_or(eyre!("unexpected kernel location"))?;

    let binary_name = binary.file_name().unwrap();
    let uefi_image_path = target_dir.join(format!("uefi-{binary_name}.img"));
    bootloader::UefiBoot::new(binary.as_std_path())
        .create_disk_image(uefi_image_path.as_std_path())
        .map_err(|err| eyre!(Box::<dyn std::error::Error + Send + Sync>::from(err)))
        .wrap_err("error creating UEFI disk image")?;
    Ok(uefi_image_path)
}
