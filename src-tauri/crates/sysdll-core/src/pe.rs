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

// Audit fix P3-5: pin the IMAGE_FILE_MACHINE constants so reviewers and
// fuzzers don't have to grep magic numbers back to the PE spec.
const MACHINE_I386: u16 = 0x014C;
const MACHINE_IA64: u16 = 0x0200;
const MACHINE_AMD64: u16 = 0x8664;
const MACHINE_ARM: u16 = 0x01C0;
const MACHINE_AARCH64: u16 = 0xAA64;

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
///
/// Audit fix P3-7: previously this used `fs::metadata(path).map(|m| m.len()).unwrap_or(0)`,
/// which silently swallowed the metadata error AND could disagree with the
/// actual byte length (race between stat and read). We now use the bytes we
/// just read so the size advertised in `PeInfo` matches the bytes analysed.
pub fn analyze_path(path: &Path) -> Result<Option<PeInfo>> {
    let bytes = fs::read(path)?;
    analyze_bytes(&bytes, bytes.len() as u64)
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
        MACHINE_I386 => "i386".into(),
        MACHINE_IA64 => "IA64".into(),
        MACHINE_AMD64 => "x86_64".into(),
        MACHINE_ARM => "ARM".into(),
        MACHINE_AARCH64 => "AArch64".into(),
        other => format!("unknown(0x{other:04X})"),
    }
}

/// Audit fix P3-6: drop the per-byte `format!` allocations in favor of a
/// single-pass hex formatter.
fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut s = String::with_capacity(digest.len() * 2);
    for byte in digest {
        // Branch-free hex without alloc.
        const HEX: &[u8; 16] = b"0123456789abcdef";
        s.push(HEX[(byte >> 4) as usize] as char);
        s.push(HEX[(byte & 0x0F) as usize] as char);
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

    #[test]
    fn sha256_hex_is_known_digest() {
        // sha256("abc") == ba7816bf...f20015ad
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn format_mach_known_ids() {
        assert_eq!(format_mach(MACHINE_AMD64), "x86_64");
        assert_eq!(format_mach(0xDEAD), "unknown(0xDEAD)");
    }
}
