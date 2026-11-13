//! Host-side detection of the tablet's USB network interface (plan §4.2).
//!
//! Nothing here sends network traffic. It only reads the interface list and,
//! on Linux, sysfs.

use std::net::Ipv4Addr;

/// The tablet's own address on the USB link.
pub const TABLET_IP: Ipv4Addr = Ipv4Addr::new(10, 11, 99, 1);

/// USB vendor:product of the reMarkable 2 Ethernet gadget, as observed.
pub const KNOWN_USB_IDS: &[(u16, u16)] = &[(0x04b3, 0x4010)];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbIds {
    pub vendor: u16,
    pub product: u16,
    pub manufacturer: Option<String>,
    pub product_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabletInterface {
    pub name: String,
    pub host_addr: Ipv4Addr,
    pub netmask: Ipv4Addr,
    /// `Some` when the OS confirmed the interface is a USB device.
    pub usb: Option<UsbIds>,
}

impl TabletInterface {
    /// The interface is a USB device with a vendor:product we recognize.
    pub fn is_known_tablet(&self) -> bool {
        self.usb
            .as_ref()
            .is_some_and(|u| KNOWN_USB_IDS.contains(&(u.vendor, u.product)))
    }
}

/// True when `addr` could be a host address handed out by the tablet.
pub fn in_tablet_subnet(addr: Ipv4Addr) -> bool {
    let o = addr.octets();
    o[0] == 10 && o[1] == 11 && o[2] == 99 && addr != TABLET_IP
}

/// Interfaces with an address in the tablet's subnet. On Linux, interfaces
/// that are not USB devices are left out. On other systems `usb` is `None`
/// and the caller must decide whether to trust a subnet match alone.
pub fn find_tablet_interfaces() -> std::io::Result<Vec<TabletInterface>> {
    let mut found = Vec::new();
    for iface in if_addrs::get_if_addrs()? {
        let if_addrs::IfAddr::V4(v4) = &iface.addr else {
            continue;
        };
        if !in_tablet_subnet(v4.ip) {
            continue;
        }
        let usb = usb_ids(&iface.name);
        if cfg!(target_os = "linux") && usb.is_none() {
            continue;
        }
        found.push(TabletInterface {
            name: iface.name.clone(),
            host_addr: v4.ip,
            netmask: v4.netmask,
            usb,
        });
    }
    Ok(found)
}

/// Names of all network interfaces that are a known reMarkable USB device,
/// whatever address they hold. Linux only; empty elsewhere.
pub fn known_usb_interfaces() -> Vec<String> {
    let mut names = Vec::new();
    if cfg!(target_os = "linux") {
        if let Ok(dir) = std::fs::read_dir("/sys/class/net") {
            for entry in dir.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let known = usb_ids(&name)
                    .is_some_and(|u| KNOWN_USB_IDS.contains(&(u.vendor, u.product)));
                if known {
                    names.push(name);
                }
            }
        }
    }
    names
}

#[cfg(target_os = "linux")]
fn usb_ids(iface: &str) -> Option<UsbIds> {
    usb_ids_in(std::path::Path::new("/sys/class/net"), iface)
}

#[cfg(not(target_os = "linux"))]
fn usb_ids(_iface: &str) -> Option<UsbIds> {
    // TODO(macOS): resolve the interface through IOKit and confirm a USB parent.
    None
}

/// Reads sysfs: `<root>/<iface>/device` points at the USB *interface*
/// directory, whose parent is the USB device holding `idVendor`/`idProduct`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn usb_ids_in(root: &std::path::Path, iface: &str) -> Option<UsbIds> {
    use std::fs;
    let dev = fs::canonicalize(root.join(iface).join("device")).ok()?;
    let subsystem = fs::canonicalize(dev.join("subsystem")).ok()?;
    if subsystem.file_name()? != "usb" {
        return None;
    }
    let usb_dev = dev.parent()?;
    let hex = |f: &str| {
        let s = fs::read_to_string(usb_dev.join(f)).ok()?;
        u16::from_str_radix(s.trim(), 16).ok()
    };
    let text = |f: &str| {
        fs::read_to_string(usb_dev.join(f))
            .ok()
            .map(|s| s.trim().to_string())
    };
    Some(UsbIds {
        vendor: hex("idVendor")?,
        product: hex("idProduct")?,
        manufacturer: text("manufacturer"),
        product_name: text("product"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subnet() {
        assert!(in_tablet_subnet(Ipv4Addr::new(10, 11, 99, 11)));
        assert!(!in_tablet_subnet(TABLET_IP));
        assert!(!in_tablet_subnet(Ipv4Addr::new(10, 11, 98, 11)));
        assert!(!in_tablet_subnet(Ipv4Addr::new(192, 168, 5, 230)));
    }

    #[cfg(unix)]
    #[test]
    fn sysfs_usb_lookup() {
        use std::fs;
        use std::os::unix::fs::symlink;
        let tmp = std::env::temp_dir().join(format!("pb-sysfs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let usb_dev = tmp.join("devices/usb1/1-5");
        let usb_if = usb_dev.join("1-5:2.0");
        fs::create_dir_all(&usb_if).unwrap();
        fs::create_dir_all(tmp.join("bus/usb")).unwrap();
        fs::create_dir_all(tmp.join("bus/pci")).unwrap();
        fs::create_dir_all(tmp.join("net/usb0")).unwrap();
        fs::create_dir_all(tmp.join("net/eth0")).unwrap();
        fs::create_dir_all(tmp.join("devices/pci0")).unwrap();
        fs::write(usb_dev.join("idVendor"), "04b3\n").unwrap();
        fs::write(usb_dev.join("idProduct"), "4010\n").unwrap();
        fs::write(usb_dev.join("manufacturer"), "reMarkable\n").unwrap();
        symlink(tmp.join("bus/usb"), usb_if.join("subsystem")).unwrap();
        symlink(&usb_if, tmp.join("net/usb0/device")).unwrap();
        symlink(tmp.join("bus/pci"), tmp.join("devices/pci0/subsystem")).unwrap();
        symlink(tmp.join("devices/pci0"), tmp.join("net/eth0/device")).unwrap();

        let ids = usb_ids_in(&tmp.join("net"), "usb0").unwrap();
        assert_eq!((ids.vendor, ids.product), (0x04b3, 0x4010));
        assert_eq!(ids.manufacturer.as_deref(), Some("reMarkable"));
        assert!(usb_ids_in(&tmp.join("net"), "eth0").is_none());
        assert!(usb_ids_in(&tmp.join("net"), "lo").is_none());
        fs::remove_dir_all(&tmp).unwrap();
    }
}
