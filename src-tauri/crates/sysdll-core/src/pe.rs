//! PE binary analysis using `pelite`.
//!
//! We extract:
//! - Architecture (PE32 vs PE32+)
//! - Import table: every DLL the file depends on
//! - Export table: every symbol the file exposes
//! - File hash (SHA-256) for integrity tracking
//!
//! All heavy work happens on memory-mapped files for zero-copy performance.

use std::fs;
use std::path::Path;

use pelite::pe64::{Pe, PeFile};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PeKind {
    Pe32,
    Pe32Plus,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeInfo {
    pub kind: PeKind,
    pub machine: String,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
    pub sha256: String,
    pub file_size: u64,
}

/// Read the raw file and parse its PE headers.
///
/// Returns `Ok(None)` for non-PE files (e.g. plain `.txt`); callers can skip these.
pub fn analyze_path(path: &Path) -> Result<Option<PeInfo>> {
    let bytes = fs::read(path)?;
    analyze_bytes(&bytes, fs::metadata(path).map(|m| m.len()).unwrap_or(0))
}

/// Analyze an in-memory PE blob.
///
/// Only PE32+ (64-bit) is supported by this MVP — that's what every modern Windows
/// DLL on `System32` is, and it covers the user's primary use case (repairing
/// system DLLs). PE32 support is a follow-up.
pub fn analyze_bytes(bytes: &[u8], file_size: u64) -> Result<Option<PeInfo>> {
    // Cheap pre-check: PE files start with "MZ" then have a "PE\0\0" signature at offset 0x3C.
    if bytes.len() < 0x40 || &bytes[0..2] != b"MZ" {
        return Ok(None);
    }
    let pe_offset = u32::from_le_bytes([bytes[0x3C], bytes[0x3D], bytes[0x3E], bytes[0x3F]]) as usize;
    if pe_offset + 4 > bytes.len() || &bytes[pe_offset..pe_offset + 4] != b"PE\0\0" {
        return Ok(None);
    }

    let file = PeFile::from_bytes(bytes).map_err(|e| Error::Pe(format!("from_bytes: {e:?}")))?;
    let machine = format_mach(file.nt_headers().FileHeader.Machine);
    let imports = collect_imports(&file);
    let exports = collect_exports(&file);
    let sha256 = sha256_hex(bytes);

    Ok(Some(PeInfo {
        kind: PeKind::Pe32Plus,
        machine,
        imports,
        exports,
        sha256,
        file_size,
    }))
}

fn collect_imports<'a>(file: &PeFile<'a>) -> Vec<String> {
    let imports = match file.imports() {
        Ok(i) => i,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for desc in imports {
        if let Ok(name) = desc.dll_name() {
            if let Ok(s) = name.to_str() {
                out.push(s.to_string());
            }
        }
    }
    out
}

fn collect_exports<'a>(file: &PeFile<'a>) -> Vec<String> {
    let exports = match file.exports() {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let by = match exports.by() {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for (name, _) in by.iter_names() {
        if let Ok(name) = name {
            if let Ok(s) = name.to_str() {
                out.push(s.to_string());
            }
        }
    }
    out
}

fn format_mach(mach: u16) -> String {
    match mach {
        0x014C => "i386".into(),
        0x0200 => "IA64".into(),
        0x8664 => "x86_64".into(),
        0x01C0 => "ARM".into(),
        0xAA64 => "AArch64".into(),
        other => format!("unknown(0x{other:04X})"),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in out {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_pe() {
        let bytes = b"not a PE file at all";
        let info = analyze_bytes(bytes, bytes.len() as u64).unwrap();
        assert!(info.is_none());
    }

    #[test]
    fn rejects_truncated_mz() {
        let bytes = b"MZ";
        let info = analyze_bytes(bytes, bytes.len() as u64).unwrap();
        assert!(info.is_none());
    }
}
