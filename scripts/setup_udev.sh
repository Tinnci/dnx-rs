#!/bin/bash

# DnX-RS USB Permissions Setup Script
# This script installs udev rules to allow non-root access to Intel DnX devices.

RULES_FILE="/etc/udev/rules.d/99-intel-dnx.rules"

echo "Creating udev rules for Intel DnX devices..."

cat <<EOF | sudo tee $RULES_FILE
# Intel Medfield/Moorefield DnX devices
SUBSYSTEM=="usb", ATTR{idVendor}=="8086", ATTR{idProduct}=="e004", MODE="0666", TAG+="uaccess"
SUBSYSTEM=="usb", ATTR{idVendor}=="8086", ATTR{idProduct}=="0a14", MODE="0666", TAG+="uaccess"
SUBSYSTEM=="usb", ATTR{idVendor}=="8086", ATTR{idProduct}=="0a2c", MODE="0666", TAG+="uaccess"
SUBSYSTEM=="usb", ATTR{idVendor}=="8086", ATTR{idProduct}=="0a65", MODE="0666", TAG+="uaccess"
EOF

echo "Reloading udev rules..."
sudo udevadm control --reload-rules
sudo udevadm trigger

echo "Done! Please replug your device."
