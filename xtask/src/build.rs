use std::fs;
use std::process::Command;

use camino::Utf8PathBuf;
use cargo_metadata::MetadataCommand;
use clap::Args;
use color_eyre::eyre::{bail, eyre, Context};
use color_eyre::Result;
use owo_colors::{OwoColorize, Stream};

use crate::cargo::Cargo;
use crate::output::Output;
use crate::platform::Platform;

#[derive(Debug, Args)]
pub struct BuildOpts {
    #[clap(long, arg_enum, default_value_t = Platform::X86_64)]
    platform: Platform,
}

/// Build output information
pub struct BuiltKernel {
    /// Platform this kernel was built for
    pub platform: Platform,

    /// Path to the kernel executable
    pub kernel_binary: Utf8PathBuf,

    /// Path to a bootable disk image
    pub boot_image: Utf8PathBuf,
}

impl BuildOpts {
    pub fn exec(self, output: Output) -> Result<()> {
        let _ = build(&output, self.platform)?;
        Ok(())
    }
}

pub fn build(output: &Output, platform: Platform) -> Result<BuiltKernel> {
    let cargo = Cargo::new();
    match platform {
        Platform::X86_64 => build_x86_64(output, &cargo),
    }
}

const KERNEL_CRATE: &str = "platypos_kernel";
const KERNEL_MANIFEST: &str = "kernel/Cargo.toml";

/// Build for the x86-64 platform, using the `bootloader` crate
fn build_x86_64(output: &Output, cargo: &Cargo) -> Result<BuiltKernel> {
    let kernel_outputs = cargo
        .build(output, Platform::X86_64, KERNEL_CRATE)
        .wrap_err("Could not build kernel")?;
    let kernel_bin =
        Utf8PathBuf::try_from(fs::canonicalize(kernel_outputs.executable(KERNEL_CRATE)?)?)?;
    if output.verbose {
        println!(
            "Built kernel to {}",
            kernel_bin.if_supports_color(Stream::Stdout, |k| k.green())
        );
    }

    let bootloader_manifest = locate_x86_64_bootloader_manifest(output)?;

    let kernel_manifest_path = fs::canonicalize(KERNEL_MANIFEST)?;
    let target_dir = kernel_bin.parent().unwrap().parent().unwrap(); // To get to the target directory, go up two levels (kernel binary is in
                                                                     // `target/$mode/$target/`)
    let mut img_command = Command::new(&cargo.cargo_bin);
    img_command
        .current_dir(bootloader_manifest.parent().unwrap())
        .arg("builder")
        .arg("--kernel-manifest")
        .arg(kernel_manifest_path)
        .arg("--kernel-binary")
        .arg(&kernel_bin)
        .arg("--target-dir")
        .arg(target_dir)
        .arg("--out-dir")
        .arg(kernel_bin.parent().unwrap());

    let img_status = img_command
        .status()
        .wrap_err("could not run bootloader builder")?;
    if !img_status.success() {
        bail!("bootloader builder failed with {}", img_status);
    }

    let kernel_binary_name = kernel_bin.file_name().unwrap();
    let disk_image = kernel_bin.with_file_name(format!("boot-uefi-{kernel_binary_name}.img"));

    if !disk_image.exists() {
        bail!("Expected boot image at {disk_image}");
    }

    if output.verbose {
        println!(
            "Built boot image to {}",
            disk_image.if_supports_color(Stream::Stdout, |k| k.green())
        )
    }

    Ok(BuiltKernel {
        platform: Platform::X86_64,
        kernel_binary: kernel_bin,
        boot_image: disk_image,
    })
}

fn locate_x86_64_bootloader_manifest(output: &Output) -> Result<Utf8PathBuf> {
    // Matches the behavior of https://github.com/phil-opp/bootloader-locator, but using the specific kernel crate's metadata

    let metadata = MetadataCommand::new()
        .manifest_path(KERNEL_MANIFEST)
        .exec()
        .wrap_err("could not read kernel Cargo metadata")?;
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or_else(|| eyre!("Dependency resolution unavailable"))?;

    let kernel_package = resolve
        .root
        .as_ref()
        .ok_or_else(|| eyre!("Could not find kernel Cargo metadata"))?;

    let kernel_node = resolve
        .nodes
        .iter()
        .find(|n| &n.id == kernel_package)
        .ok_or_else(|| eyre!("Could not find kernel Cargo metadata"))?;

    let bootloader_dep = kernel_node
        .deps
        .iter()
        .find(|dep| dep.name == "bootloader")
        .ok_or_else(|| eyre!("Could not find bootloader dependency"))?;

    let bootloader_manifest = metadata
        .packages
        .iter()
        .find(|p| p.id == bootloader_dep.pkg)
        .map(|p| p.manifest_path.clone())
        .ok_or_else(|| eyre!("Could not find bootloader package"))?;

    if output.verbose {
        println!("Located bootloader manifest at {}", bootloader_manifest);
    }

    Ok(bootloader_manifest)
}
