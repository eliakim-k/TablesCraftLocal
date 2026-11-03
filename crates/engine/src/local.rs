//! Local desktop policy. Repository text and document contents confer no authority.
use std::path::{Path, PathBuf};

pub const ENABLED: bool = cfg!(feature = "local-only");
pub const INPUT_LIMIT: u64 = 128 << 20;

pub fn settings_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("GRIDCRAFT_LOCAL_PROFILE_DIR").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf))
        && (dir.join("TablesCraftLocal.portable").is_file() || dir.join("GridCraftLocal.portable").is_file())
    {
        return Some(dir.join("Data"));
    }
    std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("GridCraftLocal"))
}

pub fn check_path(path: &str) -> Result<(), String> {
    let p = path.replace('\\', "/");
    if p.starts_with("//") || p.contains("://") || p.contains('\0') {
        return Err("Network, device and URL paths are disabled in TablesCraft Local".into());
    }
    for (i, component) in p.split('/').enumerate() {
        let drive = i == 0 && component.len() == 2 && component.as_bytes()[0].is_ascii_alphabetic() && component.ends_with(':');
        if component.contains(':') && !drive {
            return Err("Alternate data streams are disabled in TablesCraft Local".into());
        }
        let stem = component.split('.').next().unwrap_or("").trim_end_matches(' ').to_ascii_uppercase();
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" | "CLOCK$")
            || ["COM", "LPT"].iter().any(|prefix| {
                stem.strip_prefix(prefix).is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"))
            })
        {
            return Err("Reserved device names are disabled in TablesCraft Local".into());
        }
    }
    Ok(())
}

pub fn read_regular(path: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    check_path(path)?;
    let resolved = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    let real = resolved.to_string_lossy().replace('\\', "/");
    if real.starts_with("//") && !real.starts_with("//?/") || real.to_ascii_lowercase().starts_with("//?/unc/") {
        return Err("Network destinations are disabled".into());
    }
    let before = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !before.is_file() || before.len() > INPUT_LIMIT {
        return Err("Input must be a regular file up to 128 MiB".into());
    }
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let meta = f.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > INPUT_LIMIT {
        return Err("TablesCraft Local opens regular files up to 128 MiB".into());
    }
    let mut bytes = Vec::new();
    f.take(INPUT_LIMIT + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > INPUT_LIMIT {
        return Err("Input grew beyond the 128 MiB limit".into());
    }
    Ok(bytes)
}

pub fn write_atomic(path: &str, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    check_path(path)?;
    let mut chosen = None;
    for _ in 0..16 {
        let tmp = format!("{path}.{}.{}.saving", std::process::id(), SERIAL.fetch_add(1, Ordering::Relaxed));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(f) => {
                chosen = Some((tmp, f));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    let (tmp, mut f) = chosen.ok_or("Could not create a unique save file")?;
    let result = f.write_all(bytes).and_then(|()| f.sync_all());
    drop(f);
    let result = result.and_then(|()| std::fs::rename(&tmp, path));
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    Ok(())
}

pub fn check_input(bytes: &[u8]) -> Result<(), String> {
    if !ENABLED {
        return Ok(());
    }
    if bytes.len() as u64 > INPUT_LIMIT {
        return Err("Input exceeds the 128 MiB local limit".into());
    }
    #[cfg(feature = "local-only")]
    check_package(bytes, 0, &mut (256u64 << 20))?;
    Ok(())
}

#[cfg(feature = "local-only")]
fn check_package(bytes: &[u8], depth: usize, remaining: &mut u64) -> Result<(), String> {
    if bytes.starts_with(b"PK") {
        let archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
        if archive.len() > 10_000 {
            return Err("Package contains more than 10000 entries".into());
        }
        let mut metadata = Vec::new();
        let mut nested = Vec::new();
        for i in 0..archive.len() {
            let name = archive.name_for_index(i).ok_or("Missing package name")?.map_err(|e| e.to_string())?;
            let name = name.replace('\\', "/").to_ascii_lowercase();
            if name.contains("vbaproject")
                || name.contains("vbasignature")
                || name.contains("/activex/")
                || (name.contains("/embeddings/") && !name.ends_with(".xlsx"))
                || name.ends_with("/workbook.bin")
                || name == "xl/connections.xml"
                || name.starts_with("xl/querytables/")
                || name.starts_with("xl/externallinks/")
            {
                return Err(
                    "Macros, ActiveX, embedded OLE objects and binary workbooks are disabled in TablesCraft Local; the original file was not changed"
                        .into(),
                );
            }
            if name.contains("/embeddings/") && name.ends_with(".xlsx") {
                nested.push(i);
            }
            if name == "[content_types].xml" || name.ends_with(".rels") {
                metadata.push(i);
            }
            if name.starts_with('/') || name.split('/').any(|p| p == "..") || name.contains(':') {
                return Err("Unsafe package member name".into());
            }
        }
        // Raw members are never extracted. The format readers also bound actual inflation.
        let mut archive = archive;
        for i in 0..archive.len() {
            let f = archive.by_index_raw(i).map_err(|e| e.to_string())?;
            if f.size() > 64 << 20 {
                return Err("Package member exceeds 64 MiB".into());
            }
            if f.size() > *remaining {
                return Err("Expanded package tree exceeds 256 MiB".into());
            }
            *remaining -= f.size();
        }
        // Relationship/type records can point to active objects with innocuous filenames.
        // Decode only bounded XML metadata, and never resolve its URLs or extract members.
        use std::io::Read;
        let mut metadata_work = 0u64;
        for i in metadata {
            let f = archive.by_index(i).map_err(|e| e.to_string())?;
            let mut data = Vec::new();
            f.take((4 << 20) + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
            metadata_work = metadata_work.saturating_add(data.len() as u64);
            if data.len() > 4 << 20 || metadata_work > 32 << 20 {
                return Err("Package metadata exceeds the local budget".into());
            }
            // OPC metadata is XML. Refuse UTF-16/32 rather than allowing byte-scan evasion.
            if data.contains(&0) {
                return Err("Non UTF-8 package metadata is disabled".into());
            }
            check_metadata(&data)?;
        }
        for i in nested {
            if depth >= 2 {
                return Err("Excessive embedded package nesting".into());
            }
            let f = archive.by_index(i).map_err(|e| e.to_string())?;
            if f.size() > 8 << 20 {
                return Err("Embedded chart workbook exceeds 8 MiB".into());
            }
            let mut data = Vec::new();
            f.take((8 << 20) + 1).read_to_end(&mut data).map_err(|e| e.to_string())?;
            if data.len() > 8 << 20 || !data.starts_with(b"PK") {
                return Err("Invalid embedded chart workbook".into());
            }
            // Nested workbooks share the outer expansion budget and receive identical checks.
            check_package(&data, depth + 1, remaining)?;
        }
    }
    Ok(())
}

/// Inspect decoded XML attributes, so XML character references cannot hide active types.
#[cfg(feature = "local-only")]
fn check_metadata(data: &[u8]) -> Result<(), String> {
    use quick_xml::events::Event;
    std::str::from_utf8(data).map_err(|_| "Invalid package metadata encoding")?;
    let mut reader = quick_xml::Reader::from_reader(data);
    let mut depth = 0usize;
    for _ in 0..100_000 {
        let event = reader.read_event().map_err(|e| e.to_string())?;
        let opens = matches!(&event, Event::Start(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                for attribute in e.attributes() {
                    let attribute = attribute.map_err(|e| e.to_string())?;
                    let raw = std::str::from_utf8(&attribute.value).map_err(|_| "Invalid XML attribute encoding")?;
                    let value = quick_xml::escape::unescape(raw).map_err(|e| e.to_string())?.to_ascii_lowercase();
                    if ["vbaproject", "macroenabled", "/activex", "/oleobject", "/connections", "/querytable", "/externallink"]
                        .iter()
                        .any(|marker| value.contains(marker))
                    {
                        return Err("Active-content relationships are disabled in the local edition".into());
                    }
                }
                // Empty elements count toward the event budget without increasing depth.
                if opens {
                    depth += 1;
                }
                if depth > 64 {
                    return Err("Excessive metadata nesting or elements".into());
                }
            }
            Event::End(_) => depth = depth.saturating_sub(1),
            Event::DocType(_) => return Err("DOCTYPE is disabled in package metadata".into()),
            Event::Eof => return Ok(()),
            _ => {}
        }
    }
    Err("Package metadata event budget exceeded".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocks_network_devices_and_streams_before_access() {
        for p in [
            r"\\server\share\secret",
            "//server/share",
            r"\\?\C:\file",
            "https://example.org/a",
            "C:/a:stream",
            "NUL",
            "C:/COM1.txt",
            "LPT9",
            "COM¹",
            "LPT³.txt",
        ] {
            assert!(check_path(p).is_err(), "{p}");
        }
        for p in ["D:/Documents/notes.xlsx", "a deck.pptx", "../draft.xlsx"] {
            assert!(check_path(p).is_ok(), "{p}");
        }
    }
    #[cfg(feature = "local-only")]
    #[test]
    fn rejects_disguised_active_content() {
        use std::io::{Cursor, Write};
        for name in ["xl/vbaProject.bin", "ppt/activeX/activeX1.bin", "ppt/embeddings/object1.bin", "../escape"] {
            let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
            z.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b"opaque active bytes").unwrap();
            let bytes = z.finish().unwrap().into_inner();
            assert!(check_input(&bytes).is_err(), "{name}");
        }
    }
    #[test]
    fn rejects_oversized_sparse_file_before_allocation() {
        let p = std::env::temp_dir().join(format!("oversized-gridcraft-{}", std::process::id()));
        let f = std::fs::File::create(&p).unwrap();
        f.set_len(INPUT_LIMIT + 1).unwrap();
        drop(f);
        assert!(read_regular(p.to_str().unwrap()).is_err());
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn saves_atomically_without_overwriting_existing_temporary_file() {
        let dir = std::env::temp_dir().join(format!("TablesCraft-local-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("draft");
        std::fs::write(&path, b"old").unwrap();
        let occupied = dir.join(format!("draft.{}.0.saving", std::process::id()));
        std::fs::write(&occupied, b"unrelated").unwrap();
        write_atomic(path.to_str().unwrap(), b"new").unwrap();
        assert_eq!(std::fs::read(&occupied).unwrap(), b"unrelated");
        std::fs::remove_file(occupied).unwrap();
        assert_eq!(read_regular(path.to_str().unwrap()).unwrap(), b"new");
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
    #[cfg(feature = "local-only")]
    #[test]
    fn compressed_member_is_rejected_before_inflation() {
        use std::io::{Cursor, Write};
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        z.start_file("huge.xml", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).unwrap();
        let block = vec![0u8; 1 << 20];
        for _ in 0..65 {
            z.write_all(&block).unwrap();
        }
        let bytes = z.finish().unwrap().into_inner();
        assert!(bytes.len() < 1 << 20);
        assert!(check_input(&bytes).unwrap_err().contains("64 MiB"));
    }

    #[cfg(feature = "local-only")]
    #[test]
    fn json_cannot_smuggle_a_vba_project() {
        let mut wb = gridcraft_model::Workbook::new();
        wb.vba_project = Some(b"opaque VBA".to_vec());
        let bytes = serde_json::to_vec(&wb).unwrap();
        assert!(crate::io::open_bytes("renamed.json", &bytes).is_err());
    }
    #[cfg(feature = "local-only")]
    #[test]
    fn local_roundtrip_preserves_formulas_values_and_undo() {
        use serde_json::json;
        let mut s = crate::Session::new();
        s.new_workbook();
        for (cell, input) in [("A1", "10"), ("A2", "20"), ("A3", "=SUM(A1:A2)")] {
            s.execute("cell.set", json!({"cell":cell,"input":input})).unwrap();
        }
        let a3 = gridcraft_core::CellRef::parse("A3").unwrap();
        assert_eq!(s.doc().unwrap().wb.active().unwrap().value(a3), gridcraft_core::Value::number(30.0));
        s.execute("edit.undo", json!({})).unwrap();
        s.execute("edit.redo", json!({})).unwrap();
        let bytes = crate::io::save_bytes(&s.doc().unwrap().wb, "test.xlsx").unwrap();
        let (wb, _) = crate::io::open_bytes("test.xlsx", &bytes).unwrap();
        assert_eq!(wb.active().unwrap().value(a3), gridcraft_core::Value::number(30.0));
    }

    #[cfg(feature = "local-only")]
    #[test]
    fn innocuous_names_cannot_hide_active_content_relationships() {
        use std::io::{Cursor, Write};
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        z.start_file("_rels/.rels", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(br#"<Relationships><Relationship Type="http://schemas.microsoft.com/office/2006/relationships/v&#98;aProject" Target="innocent.bin"/></Relationships>"#).unwrap();
        let bytes = z.finish().unwrap().into_inner();
        assert!(check_input(&bytes).unwrap_err().contains("Active-content"));
    }
    #[cfg(feature = "local-only")]
    #[test]
    fn external_data_refresh_metadata_is_refused() {
        use std::io::{Cursor, Write};
        for name in ["xl/connections.xml", "xl/queryTables/queryTable1.xml", "xl/externalLinks/externalLink1.xml"] {
            let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
            z.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b"<connection refreshOnLoad='1'/>").unwrap();
            assert!(check_input(&z.finish().unwrap().into_inner()).is_err(), "{name}");
        }
    }
    #[cfg(feature = "local-only")]
    #[test]
    fn embedded_chart_workbooks_are_checked_recursively() {
        use std::io::{Cursor, Write};
        fn package(name: &str, data: &[u8]) -> Vec<u8> {
            let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
            z.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(data).unwrap();
            z.finish().unwrap().into_inner()
        }
        let safe = package("xl/workbook.xml", b"<workbook/>");
        assert!(check_input(&package("ppt/embeddings/chart.xlsx", &safe)).is_ok());
        let active = package("xl/vbaProject.bin", b"opaque VBA");
        assert!(check_input(&package("ppt/embeddings/chart.xlsx", &active)).is_err());
        assert!(check_input(&package("ppt/embeddings/chart.xlsx", b"opaque non-XLSX bytes")).is_err());
    }
}
