use std::path::PathBuf;
use std::process::Command;
use std::fs::{self, File};

use failure::{Error, bail};
use reqwest;
use tempdir::TempDir;
use cargo_metadata::Metadata;

use crate::step_style;

/// URL to the RPM containing the OVMF RPM. This needs to be updated periodically to the latest version
const OVMF_RPM_URL: &str = "https://www.kraxel.org/repos/jenkins/edk2/edk2.git-ovmf-x64-0-20190703.1160.g03835a8c73.noarch.rpm";


/// OVMF firmware download
#[derive(Debug, Clone)]
pub struct Ovmf {
    /// Directory where the firmware files are located
    base_dir: PathBuf,
}

impl Ovmf {
    pub fn create(metadata: &Metadata) -> Result<Ovmf, Error> {
        let ovmf = Ovmf {
            base_dir: metadata.target_directory.join("ovmf")
        };

        if !ovmf.is_downloaded() {
            ovmf.download()?;
        }

        Ok(ovmf)
    }

    fn is_downloaded(&self) -> bool {
        self.firmware().exists() && self.vars_template().exists()
    }

    /// The OVMF_CODE.fd file, containing the OVMF firmware
    pub fn firmware(&self) -> PathBuf {
        self.base_dir.join("OVMF_CODE.fd")
    }

    /// The OVMF_VARS.fd file, which is a template for the NVRAM firmware variable storage
    pub fn vars_template(&self) -> PathBuf {
        self.base_dir.join("OVMF_VARS.fd")
    }

    fn download(&self) -> Result<(), Error> {
        println!("{}", step_style().paint("Downloading OVMF"));
        fs::create_dir_all(&self.base_dir)?;

        let working_dir = TempDir::new("ovmf")?;

        let mut rpm_file = File::create(working_dir.path().join("edk2-ovmf.rpm"))?;
        reqwest::get(OVMF_RPM_URL)?
            .error_for_status()?
            .copy_to(&mut rpm_file)?;

        let mut tar_command = Command::new("tar")
            .args(&["-x", "-f", "edk2-ovmf.rpm"])
            .current_dir(working_dir.path())
            .spawn()?;
        let status = tar_command.wait()?;
        if !status.success() {
            bail!("Failed to extract OVMF RPM");
        }

        let firmware_file = working_dir.path().join("usr/share/edk2.git/ovmf-x64/OVMF_CODE-pure-efi.fd");
        let vars_file = working_dir.path().join("usr/share/edk2.git/ovmf-x64/OVMF_VARS-pure-efi.fd");

        fs::rename(firmware_file, self.firmware())?;
        fs::rename(vars_file, self.vars_template())?;

        Ok(())

    }
}
