//! Chaabi firmware helper functions.

/// Helper to find Chaabi range in DnX binary.
/// Returns (start, end) offsets for the Token+FW section (NOT including CDPH).
pub fn find_chaabi_range(data: &[u8]) -> Option<(usize, usize)> {
    let ch00_magic = b"CH00";
    let cdph_magic = b"CDPH";
    let dtkn_magic = b"DTKN";

    let find = |needle: &[u8]| -> Option<usize> {
        data.windows(needle.len())
            .position(|window| window == needle)
    };

    let ch00_pos = find(ch00_magic)?;
    let cdph_pos = find(cdph_magic)?;

    // Token+FW start: CH00 - 0x80, or DTKN if found
    let mut start = ch00_pos.checked_sub(0x80)?;
    if let Some(dtkn_pos) = data[..ch00_pos]
        .windows(dtkn_magic.len())
        .position(|w| w == dtkn_magic)
    {
        start = dtkn_pos;
    }

    // Token+FW end: CDPH position (CDPH is separate)
    let end = cdph_pos;

    if start < end && end <= data.len() {
        Some((start, end))
    } else {
        None
    }
}

/// Build Chaabi payload with correct structure for device.
/// According to xFSTK's InitDnx(), the structure is:
/// [CDPH Header (24 bytes from FILE END)] + [Token + FW data]
///
/// **NOTE**: This file has 488 extra bytes after CDPH, so we use magic string positions
/// instead of xFSTK's (file_size - token - fw - 24) calculation.
pub fn build_chaabi_payload(data: &[u8]) -> Option<Vec<u8>> {
    let ch00_magic = b"CH00";
    let cdph_magic = b"CDPH";
    let dtkn_magic = b"DTKN";
    let cht_magic = b"$CHT"; // TNG A0
    let chpr_magic = b"ChPr"; // TNG B0/ANN

    let find = |needle: &[u8]| -> Option<usize> {
        data.windows(needle.len())
            .position(|window| window == needle)
    };

    let ch00_pos = find(ch00_magic)?;
    let cdph_pos = find(cdph_magic)?;
    let file_size = data.len();

    // Calculate CH00 adjusted position (used for fallback)
    let ch00_adjusted = ch00_pos.checked_sub(0x80)?;

    // Determine Token+FW start position based on magic string markers
    // Priority: DTKN > $CHT > ChPr > CH00-0x80
    let token_fw_start = if let Some(dtkn_pos) = find(dtkn_magic) {
        if dtkn_pos < ch00_pos {
            tracing::info!("Using DTKN marker at 0x{:x} for Token start", dtkn_pos);
            dtkn_pos
        } else {
            ch00_adjusted
        }
    } else if let Some(cht_pos) = find(cht_magic) {
        if cht_pos < ch00_pos {
            let start = cht_pos.checked_sub(0x80)?;
            tracing::info!(
                "Using $CHT marker at 0x{:x}, Token starts at 0x{:x}",
                cht_pos,
                start
            );
            start
        } else {
            ch00_adjusted
        }
    } else if let Some(chpr_pos) = find(chpr_magic) {
        if chpr_pos < ch00_pos {
            tracing::info!("Using ChPr marker at 0x{:x} for Token start", chpr_pos);
            chpr_pos
        } else {
            ch00_adjusted
        }
    } else {
        tracing::info!("No token marker found, using CH00 - 0x80");
        ch00_adjusted
    };

    // Token+FW end: CDPH magic string position (NOT file end!)
    let token_fw_end = cdph_pos;
    let token_fw_size = token_fw_end.saturating_sub(token_fw_start);

    tracing::info!(
        "Chaabi Token+FW: 0x{:x} to 0x{:x} ({} bytes)",
        token_fw_start,
        token_fw_end,
        token_fw_size
    );

    // Validate bounds
    if token_fw_start >= token_fw_end || token_fw_end > file_size {
        tracing::warn!("Invalid Token+FW range!");
        return None;
    }

    // CDPH header: LAST 24 bytes of the FILE (not from CDPH string position!)
    if file_size < 24 {
        return None;
    }
    let cdph_header = &data[file_size - 24..file_size];
    let token_fw_data = &data[token_fw_start..token_fw_end];

    // Build: CDPH first (from file end), then Token+FW
    let mut payload = Vec::with_capacity(24 + token_fw_size);
    payload.extend_from_slice(cdph_header);
    payload.extend_from_slice(token_fw_data);

    tracing::info!(
        "Built Chaabi payload: {} bytes (header 24 + body {})",
        payload.len(),
        token_fw_size
    );

    Some(payload)
}
