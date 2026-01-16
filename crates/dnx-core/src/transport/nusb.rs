//! nusb-based USB transport implementation.

use nusb::transfer::{Bulk, In, Out};
use nusb::{Interface, MaybeFuture, list_devices};
use std::io::{Read, Write};
use tracing::{debug, info, instrument};

use super::traits::{TransportError, UsbTransport};
use crate::protocol::AckCode;
use crate::protocol::constants::{INTEL_VENDOR_ID, SUPPORTED_PIDS};

/// nusb-based USB transport.
pub struct NusbTransport {
    interface: Interface,
    in_endpoint: u8,
    out_endpoint: u8,
    vid: u16,
    pid: u16,
}

impl NusbTransport {
    /// Open any matching Intel DnX device (tries all supported PIDs).
    #[instrument(level = "info")]
    pub fn open() -> Result<Self, TransportError> {
        let devices = list_devices()
            .wait()
            .map_err(|e| TransportError::OpenFailed(e.to_string()))?;

        // Try to find any Intel device with a supported PID
        for device_info in devices {
            if device_info.vendor_id() == INTEL_VENDOR_ID
                && SUPPORTED_PIDS.contains(&device_info.product_id())
            {
                return Self::open_device_info(device_info);
            }
        }

        Err(TransportError::DeviceNotFound {
            vid: INTEL_VENDOR_ID,
            pid: 0,
        })
    }

    /// Open a device with specific VID/PID.
    #[instrument(level = "info", fields(vid = format!("{:04X}", vid), pid = format!("{:04X}", pid)))]
    pub fn open_with_ids(vid: u16, pid: u16) -> Result<Self, TransportError> {
        let device_info = list_devices()
            .wait()
            .map_err(|e| TransportError::OpenFailed(e.to_string()))?
            .find(|d| d.vendor_id() == vid && d.product_id() == pid)
            .ok_or(TransportError::DeviceNotFound { vid, pid })?;

        Self::open_device_info(device_info)
    }

    fn open_device_info(device_info: nusb::DeviceInfo) -> Result<Self, TransportError> {
        let vid = device_info.vendor_id();
        let pid = device_info.product_id();

        info!(
            vendor_id = %format!("{:04X}", vid),
            product_id = %format!("{:04X}", pid),
            "Found device"
        );

        let device = device_info
            .open()
            .wait()
            .map_err(|e| TransportError::OpenFailed(e.to_string()))?;

        let interface =
            device
                .claim_interface(0)
                .wait()
                .map_err(|e| TransportError::ClaimInterfaceFailed {
                    interface: 0,
                    message: e.to_string(),
                })?;

        // Find BULK endpoints
        let mut in_endpoint: u8 = 0;
        let mut out_endpoint: u8 = 0;

        for config in device.configurations() {
            for iface in config.interfaces() {
                if iface.interface_number() == 0 {
                    for alt in iface.alt_settings() {
                        for ep in alt.endpoints() {
                            if ep.transfer_type() == nusb::descriptors::TransferType::Bulk {
                                if ep.direction() == nusb::transfer::Direction::In {
                                    in_endpoint = ep.address();
                                } else {
                                    out_endpoint = ep.address();
                                }
                            }
                        }
                    }
                }
            }
        }

        if in_endpoint == 0 {
            return Err(TransportError::EndpointNotFound {
                ep_type: "Bulk".into(),
                direction: "In".into(),
            });
        }
        if out_endpoint == 0 {
            return Err(TransportError::EndpointNotFound {
                ep_type: "Bulk".into(),
                direction: "Out".into(),
            });
        }

        info!(
            in_ep = %format!("0x{:02X}", in_endpoint),
            out_ep = %format!("0x{:02X}", out_endpoint),
            "Device opened successfully"
        );

        Ok(Self {
            interface,
            in_endpoint,
            out_endpoint,
            vid,
            pid,
        })
    }
}

impl UsbTransport for NusbTransport {
    #[instrument(skip(self, data), fields(len = data.len()))]
    fn write(&self, data: &[u8]) -> Result<usize, TransportError> {
        let ep = self
            .interface
            .endpoint::<Bulk, Out>(self.out_endpoint)
            .map_err(|e| TransportError::WriteFailed(e.to_string()))?;

        let mut writer = ep.writer(4096);
        writer
            .write_all(data)
            .map_err(|e| TransportError::WriteFailed(e.to_string()))?;
        writer
            .flush()
            .map_err(|e| TransportError::WriteFailed(e.to_string()))?;

        debug!(bytes_written = data.len(), "Write complete");
        Ok(data.len())
    }

    #[instrument(skip(self), fields(max_len))]
    fn read(&self, max_len: usize) -> Result<Vec<u8>, TransportError> {
        let ep = self
            .interface
            .endpoint::<Bulk, In>(self.in_endpoint)
            .map_err(|e| TransportError::ReadFailed(e.to_string()))?;

        let mut reader = ep.reader(4096);
        let mut buf = vec![0u8; max_len];

        let n = reader
            .read(&mut buf)
            .map_err(|e| TransportError::ReadFailed(e.to_string()))?;

        buf.truncate(n);
        debug!(bytes_read = n, "Read complete");
        Ok(buf)
    }

    fn read_ack(&self) -> Result<AckCode, TransportError> {
        let bytes = self.read(512)?;
        if bytes.is_empty() {
            return Err(TransportError::ReadFailed("Empty ACK response".into()));
        }
        Ok(AckCode::from_bytes(&bytes))
    }

    fn is_connected(&self) -> bool {
        // nusb doesn't provide a direct "is connected" check.
        // We could try a zero-length read, but for now just return true.
        true
    }

    fn vendor_id(&self) -> u16 {
        self.vid
    }

    fn product_id(&self) -> u16 {
        self.pid
    }
}
