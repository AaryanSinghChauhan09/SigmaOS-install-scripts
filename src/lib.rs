// SPDX-License-Identifier: Apache-2.0
//! SigmaOS-install-scripts — scripted installation profiles, Arch-inspired.
//!
//! Models `archinstall`-style profiles: a disk partitioning plan, package
//! selection, bootloader choice, and firmware mode. Validation is fail-fast —
//! an install profile that can't produce a bootable system is rejected.

use std::fmt;

/// Errors validating an install profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    /// No root (`/`) partition defined.
    MissingRoot,
    /// BIOS firmware requires a bootloader; UEFI uses systemd-boot entries.
    MissingBootloader,
    /// Empty package list — nothing to install.
    EmptyPackages,
    /// Partition mountpoint is empty or duplicated.
    BadPartition(String),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRoot => write!(f, "no root partition defined"),
            Self::MissingBootloader => write!(f, "BIOS install requires a bootloader"),
            Self::EmptyPackages => write!(f, "package list is empty"),
            Self::BadPartition(p) => write!(f, "bad partition: {p}"),
        }
    }
}

impl std::error::Error for InstallError {}

/// Firmware mode of the target machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Firmware {
    /// Legacy BIOS — needs a bootloader (grub, syslinux).
    Bios,
    /// UEFI — boots via EFI system partition entries.
    Uefi,
}

/// A single disk partition in the install plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    /// Mountpoint (e.g. `/`, `/boot`, `/home`).
    pub mountpoint: String,
    /// Size in GiB.
    pub size_gib: u32,
    /// Filesystem type.
    pub fs: String,
}

/// A complete, validated installation profile.
#[derive(Debug, Clone)]
pub struct InstallProfile {
    /// Hostname for the installed system.
    pub hostname: String,
    /// Partitioning plan.
    pub partitions: Vec<Partition>,
    /// Packages to install (pacman groups/packages).
    pub packages: Vec<String>,
    /// Bootloader binary name, if any.
    pub bootloader: Option<String>,
    /// Target firmware.
    pub firmware: Firmware,
}

impl InstallProfile {
    /// Validate the profile; returns fail-fast errors for unbootable configs.
    pub fn validate(&self) -> Result<(), InstallError> {
        if !self
            .partitions
            .iter()
            .any(|p| p.mountpoint == "/")
        {
            return Err(InstallError::MissingRoot);
        }
        if self.firmware == Firmware::Bios && self.bootloader.is_none() {
            return Err(InstallError::MissingBootloader);
        }
        if self.packages.is_empty() {
            return Err(InstallError::EmptyPackages);
        }
        let mut seen = std::collections::HashSet::new();
        for p in &self.partitions {
            if p.mountpoint.trim().is_empty() || !seen.insert(p.mountpoint.clone()) {
                return Err(InstallError::BadPartition(p.mountpoint.clone()));
            }
        }
        Ok(())
    }

    /// Convenience: does the plan include an ESP for UEFI?
    pub fn has_esp(&self) -> bool {
        self.partitions
            .iter()
            .any(|p| p.mountpoint == "/boot" || p.mountpoint == "/efi")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_profile() -> InstallProfile {
        InstallProfile {
            hostname: "sigma".into(),
            partitions: vec![
                Partition { mountpoint: "/".into(), size_gib: 40, fs: "btrfs".into() },
                Partition { mountpoint: "/boot".into(), size_gib: 1, fs: "vfat".into() },
            ],
            packages: vec!["base".into(), "linux".into(), "mkinitcpio".into()],
            bootloader: Some("grub".into()),
            firmware: Firmware::Bios,
        }
    }

    #[test]
    fn valid_profile_passes() {
        assert!(base_profile().validate().is_ok());
    }

    #[test]
    fn missing_root_fails() {
        let mut p = base_profile();
        p.partitions.retain(|x| x.mountpoint != "/");
        assert_eq!(p.validate(), Err(InstallError::MissingRoot));
    }

    #[test]
    fn bios_without_bootloader_fails() {
        let mut p = base_profile();
        p.bootloader = None;
        assert_eq!(p.validate(), Err(InstallError::MissingBootloader));
    }

    #[test]
    fn uefi_without_bootloader_ok() {
        let mut p = base_profile();
        p.firmware = Firmware::Uefi;
        p.bootloader = None;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn empty_packages_fail() {
        let mut p = base_profile();
        p.packages.clear();
        assert_eq!(p.validate(), Err(InstallError::EmptyPackages));
    }

    #[test]
    fn duplicate_mountpoints_fail() {
        let mut p = base_profile();
        p.partitions.push(Partition {
            mountpoint: "/".into(),
            size_gib: 10,
            fs: "ext4".into(),
        });
        assert!(matches!(p.validate(), Err(InstallError::BadPartition(_))));
    }
}
