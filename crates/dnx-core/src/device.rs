//! USB device abstraction using nusb (Pure Rust, async-first).
//!
//! Key nusb 0.2 API patterns:
//! - `list_devices().wait()` or `.await` for device enumeration
//! - `device_info.open().wait()` or `.await` to open device
//! - `device.claim_interface(n).wait()` or `.await` to claim interface
//! - `interface.endpoint::<Bulk, Out>(addr)` to get endpoint
//! - endpoint `.writer(buf_size)` / `.reader(buf_size)` for I/O

use anyhow::{Result, anyhow};
use nusb::transfer::{Bulk, In, Out};
use nusb::{Interface, MaybeFuture, list_devices};
use std::io::{Read, Write};
use tracing::{debug, info, instrument};

use crate::protocol::{INTEL_VENDOR_ID, MEDFIELD_PRODUCT_ID};

pub struct DnxDevice {
    interface: Interface,
    in_endpoint: u8,
    out_endpoint: u8,
}

impl DnxDevice {
    /// Open and claim the first matching Intel Medfield device.
    /// This function uses `.wait()` for blocking behavior suitable for sync contexts,
    /// but the struct methods for I/O are blocking (std::io::Read/Write).
    #[instrument(level = "info")]
    pub fn open() -> Result<Self> {
        // list_devices() returns impl MaybeFuture, use .wait() to block
        let device_info = list_devices()
            .wait()?
            .find(|d| d.vendor_id() == INTEL_VENDOR_ID && d.product_id() == MEDFIELD_PRODUCT_ID)
            .ok_or_else(|| {
                debug!("No suitable device found");
                anyhow!(
                    "Device not found: VID={:04x} PID={:04x}",
                    INTEL_VENDOR_ID,
                    MEDFIELD_PRODUCT_ID
                )
            })?;

        info!(
            vendor_id = %format!("{:04x}", device_info.vendor_id()),
            product_id = %format!("{:04x}", device_info.product_id()),
            "Found Medfield device"
        );

        // Open device (blocking)
        let device = device_info.open().wait()?;

        // Claim interface 0 (blocking)
        let interface = device.claim_interface(0).wait()?;

        // Find BULK endpoints by iterating configurations
        let mut in_endpoint: u8 = 0;
        let mut out_endpoint: u8 = 0;

        for config in device.configurations() {
            for iface in config.interfaces() {
                if iface.interface_number() == 0 {
                    for alt in iface.alt_settings() {
                        for ep in alt.endpoints() {
                            // TransferType is in nusb::descriptors module
                            if ep.transfer_type() == nusb::descriptors::TransferType::Bulk {
                                let addr = ep.address();
                                // Use direction() method which returns nusb::transfer::Direction
                                if ep.direction() == nusb::transfer::Direction::In {
                                    in_endpoint = addr;
                                } else {
                                    out_endpoint = addr;
                                }
                            }
                        }
                    }
                }
            }
        }

        if in_endpoint == 0 || out_endpoint == 0 {
            return Err(anyhow!("Could not find Bulk IN/OUT endpoints"));
        }

        info!(in_ep = %format!("0x{:02x}", in_endpoint), out_ep = %format!("0x{:02x}", out_endpoint), "Device opened successfully");

        Ok(DnxDevice {
            interface,
            in_endpoint,
            out_endpoint,
        })
    }

    /// Write data to the device using Bulk OUT endpoint.
    #[instrument(skip(self, data), fields(len = data.len()))]
    pub fn write(&self, data: &[u8]) -> Result<usize> {
        // Get a writer for the OUT endpoint
        // endpoint::<Bulk, Out>(addr) returns Result<Endpoint<Bulk, Out>, Error>
        let ep = self.interface.endpoint::<Bulk, Out>(self.out_endpoint)?;
        let mut writer = ep.writer(4096); // buffer size

        writer.write_all(data)?;
        writer.flush()?;

        debug!(bytes_written = %data.len(), "Write complete");
        Ok(data.len())
    }

    /// Read data from the device using Bulk IN endpoint.
    #[instrument(skip(self), fields(max_len = len))]
    pub fn read(&self, len: usize) -> Result<Vec<u8>> {
        let ep = self.interface.endpoint::<Bulk, In>(self.in_endpoint)?;
        let mut reader = ep.reader(4096);

        let mut buf = vec![0u8; len];
        let n = reader.read(&mut buf)?;
        buf.truncate(n);

        debug!(bytes_read = %n, "Read complete");
        Ok(buf)
    }

    /// Read ACK response (typically up to 512 bytes).
    pub fn read_ack(&self) -> Result<Vec<u8>> {
        self.read(512)
    }
}
