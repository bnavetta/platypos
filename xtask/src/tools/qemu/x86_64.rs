//! Platform-specific QEMU setup code for x86-64

use cargo_metadata::MetadataCommand;

use crate::prelude::*;
use crate::tools::cargo::{self, Cargo};

pub fn build_boot_image(crate_name: &str, binary: &Utf8Path, cargo: &Cargo) -> Result<Utf8PathBuf> {
    let crate_manifest = cargo::manifest_path(crate_name);
    let bootloader_manifest = locate_bootloader_manifest(&crate_manifest)?;

    let target_dir = binary.parent().unwrap().parent().unwrap(); // To get to the target directory, go up two levels (kernel binary is in
                                                                 // `target/$mode/$target/`)
    let img_status = cargo
        .command()
        .current_dir(bootloader_manifest.parent().unwrap())
        .arg("builder")
        .arg("--kernel-manifest")
        .arg(crate_manifest.canonicalize()?)
        .arg("--kernel-binary")
        .arg(&binary)
        .arg("--target-dir")
        .arg(target_dir)
        .arg("--out-dir")
        .arg(binary.parent().unwrap())
        .status()
        .wrap_err("could not run bootloader builder")?;

    if !img_status.success() {
        bail!("bootloader builder failed with {img_status}");
    }

    let binary_name = binary.file_name().unwrap();
    let disk_image = binary.with_file_name(format!("boot-uefi-{binary_name}.img"));
    if !disk_image.exists() {
        bail!("Expected boot image at {disk_image}");
    }

    log::info!(
        "Built boot image to {}",
        disk_image.if_supports_color(Stream::Stdout, |k| k.magenta())
    );

    Ok(disk_image)
}

/// Locate the Cargo.toml file for the bootloader
fn locate_bootloader_manifest(crate_manifest: &Utf8Path) -> Result<Utf8PathBuf> {
    // Matches the behavior of https://github.com/phil-opp/bootloader-locator, but using the specific kernel crate's metadata

    let metadata = MetadataCommand::new()
        .manifest_path(crate_manifest)
        .exec()
        .wrap_err("could not read {crate_manifest}")?;
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or_else(|| eyre!("Dependency resolution unavailable"))?;

    let package = resolve
        .root
        .as_ref()
        .ok_or_else(|| eyre!("Could not find root metadata in {crate_manifest}"))?;

    let node = resolve
        .nodes
        .iter()
        .find(|n| &n.id == package)
        .ok_or_else(|| eyre!("Could not find dependency metadata in {crate_manifest}"))?;

    let bootloader_dep = node
        .deps
        .iter()
        .find(|dep| dep.name == "bootloader")
        .ok_or_else(|| eyre!("{crate_manifest} does not depend on bootloader"))?;

    let bootloader_manifest = metadata
        .packages
        .iter()
        .find(|p| p.id == bootloader_dep.pkg)
        .map(|p| p.manifest_path.clone())
        .ok_or_else(|| eyre!("Could not find bootloader package"))?;

    log::debug!("Located bootloader manifest at {}", bootloader_manifest);

    Ok(bootloader_manifest)
}
