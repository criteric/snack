//! Snack .yummy v1 archive: custom, non-ZIP binary container.
//! Content integrity is SHA-256; this is NOT a digital signature.
use sha2::{Digest, Sha256};
use std::{fs, io::{self, Read, Write}, path::{Component, Path}};

const MAGIC: &[u8; 9] = b"SNACKYUM\x01";
const MAX_ARCHIVE: u64 = 512 * 1024 * 1024;
const MAX_ENTRY: u64 = 128 * 1024 * 1024;
const MAX_COUNT: u32 = 2048;

#[derive(Clone, Debug)]
pub struct Entry { pub name: String, pub data: Vec<u8> }

fn invalid(message: &str) -> io::Error { io::Error::new(io::ErrorKind::InvalidData, message.to_string()) }

fn valid_name(s: &str) -> io::Result<()> {
    if s.is_empty() || s.len() > 1024 || s.contains('\\') || s.contains('\0') || s.contains(':') {
        return Err(invalid("unsafe archive entry name"));
    }
    if s.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
        return Err(invalid("archive contains an unsafe path segment"));
    }
    let mut components = 0;
    for c in Path::new(s).components() {
        match c { Component::Normal(_) => components += 1, _ => return Err(invalid("archive contains an unsafe path")) }
    }
    if components == 0 { return Err(invalid("empty archive entry")); }
    Ok(())
}

fn collect(base: &Path, root: &Path, result: &mut Vec<Entry>) -> io::Result<()> {
    for item in fs::read_dir(base)? {
        let item = item?;
        let p = item.path();
        let name = item.file_name().to_string_lossy().into_owned();
        if ["target", ".git", "dist", ".snack-backups"].contains(&name.as_str()) { continue; }
        let kind = fs::symlink_metadata(&p)?.file_type();
        if kind.is_symlink() { return Err(invalid("symlinks are not allowed in .yummy packages")); }
        if kind.is_dir() { collect(&p, root, result)?; }
        else if kind.is_file() {
            let relative = p.strip_prefix(root).map_err(|_| invalid("invalid project root"))?;
            let n = relative.to_string_lossy().replace('\\', "/");
            valid_name(&n)?;
            if fs::metadata(&p)?.len() > MAX_ENTRY { return Err(invalid("file too large")); }
            let data = fs::read(&p)?;
            result.push(Entry { name: n, data });
            if result.len() > MAX_COUNT as usize { return Err(invalid("too many archive entries")); }
        }
    }
    Ok(())
}

pub fn pack(project: &Path, destination: &Path) -> io::Result<()> {
    if !project.join("manifest.toml").is_file() { return Err(invalid("manifest.toml is required")); }
    let mut entries = Vec::new();
    collect(project, project, &mut entries)?;
    entries.sort_by(|a,b| a.name.cmp(&b.name));
    let total: u64 = entries.iter().map(|e| e.data.len() as u64).sum();
    if total > MAX_ARCHIVE { return Err(invalid("package too large")); }
    if entries.len() > MAX_COUNT as usize { return Err(invalid("too many entries")); }
    let tmp = destination.with_extension("yummy.tmp");
    if let Some(parent) = tmp.parent() { fs::create_dir_all(parent)?; }
    let mut out = fs::File::create(&tmp)?;
    out.write_all(MAGIC)?;
    out.write_all(&(entries.len() as u32).to_le_bytes())?;
    for entry in entries {
        let n = entry.name.as_bytes();
        if n.len() > u16::MAX as usize { return Err(invalid("name too long")); }
        out.write_all(&(n.len() as u16).to_le_bytes())?;
        out.write_all(n)?;
        out.write_all(&(entry.data.len() as u64).to_le_bytes())?;
        out.write_all(&Sha256::digest(&entry.data))?;
        out.write_all(&entry.data)?;
    }
    out.sync_all()?;
    fs::rename(tmp, destination)?;
    Ok(())
}

pub fn read(path: &Path) -> io::Result<Vec<Entry>> {
    let len = fs::metadata(path)?.len();
    if len > MAX_ARCHIVE + 1024 * 1024 { return Err(invalid("archive too large")); }
    let mut input = fs::File::open(path)?;
    let mut magic = [0u8; 9]; input.read_exact(&mut magic)?;
    if &magic != MAGIC { return Err(invalid("invalid .yummy header or version")); }
    let mut nbuf = [0u8; 4]; input.read_exact(&mut nbuf)?;
    let count = u32::from_le_bytes(nbuf);
    if count == 0 || count > MAX_COUNT { return Err(invalid("invalid entry count")); }
    let mut entries = Vec::new();
    let mut total = 0u64;
    let mut seen = std::collections::HashSet::new();
    for _ in 0..count {
        let mut short = [0u8; 2]; input.read_exact(&mut short)?;
        let nlen = u16::from_le_bytes(short) as usize;
        if nlen == 0 || nlen > 1024 { return Err(invalid("invalid filename length")); }
        let mut name_bytes = vec![0u8; nlen]; input.read_exact(&mut name_bytes)?;
        let name = String::from_utf8(name_bytes).map_err(|_| invalid("archive path must be UTF-8"))?;
        valid_name(&name)?;
        if !seen.insert(name.clone()) { return Err(invalid("duplicate file in package")); }
        let mut size = [0u8; 8]; input.read_exact(&mut size)?;
        let bytes = u64::from_le_bytes(size);
        total = total.checked_add(bytes).ok_or_else(|| invalid("size overflow"))?;
        if bytes > MAX_ENTRY || total > MAX_ARCHIVE { return Err(invalid("archive size limit exceeded")); }
        let mut sha = [0u8; 32]; input.read_exact(&mut sha)?;
        let mut data = vec![0u8; bytes as usize]; input.read_exact(&mut data)?;
        if &Sha256::digest(&data)[..] != &sha[..] { return Err(invalid("SHA-256 checksum mismatch")); }
        entries.push(Entry {name, data});
    }
    let mut last = [0u8; 1];
    if input.read(&mut last)? != 0 { return Err(invalid("unexpected trailing data")); }
    if !entries.iter().any(|e| e.name == "manifest.toml") { return Err(invalid("missing manifest.toml")); }
    Ok(entries)
}

pub fn manifest(entries: &[Entry]) -> io::Result<String> {
    let m = entries.iter().find(|e| e.name == "manifest.toml").ok_or_else(|| invalid("missing manifest"))?;
    String::from_utf8(m.data.clone()).map_err(|_| invalid("manifest must be UTF-8"))
}

pub fn extract(entries: &[Entry], destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    let base = fs::canonicalize(destination)?;
    for e in entries {
        valid_name(&e.name)?;
        let path = base.join(&e.name);
        // Walk the destination before creating anything; never traverse symlinks.
        let mut folder = base.clone();
        let parts: Vec<_> = Path::new(&e.name).components().collect();
        for c in parts.iter().take(parts.len().saturating_sub(1)) {
            if let Component::Normal(segment) = c {
                folder.push(segment);
                match fs::symlink_metadata(&folder) {
                    Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => return Err(invalid("unsafe extraction directory")),
                    Ok(_) => {},
                    Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir(&folder)?,
                    Err(e) => return Err(e),
                }
            }
        }
        if path.exists() || fs::symlink_metadata(&path).is_ok() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("refusing to overwrite {}", path.display())));
        }
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
        file.write_all(&e.data)?;
    }
    Ok(())
}

pub fn manifest_fields(s: &str) -> io::Result<std::collections::HashMap<String, String>> {
    let table: toml::Value = s.parse().map_err(|e| invalid(&format!("invalid manifest.toml: {e}")))?;
    let fields = table.get("plugin").and_then(|v| v.as_table()).ok_or_else(|| invalid("missing [plugin] table"))?;
    let mut out: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for key in ["id", "name", "version", "sdk", "runtime", "language", "activation"] {
        if let Some(s) = fields.get(key).and_then(|v| v.as_str()) { out.insert(key.into(), s.into()); }
    }
    for key in ["id", "name", "version", "sdk", "runtime"] {
        if !out.contains_key(key) { return Err(invalid(&format!("missing plugin.{key}"))); }
    }
    if out["id"].is_empty() || !out["id"].chars().all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c)) {
        return Err(invalid("plugin.id must contain only letters, numbers, '.', '-', '_'"));
    }
    let version = semver::Version::parse(&out["version"]).map_err(|e| invalid(&format!("invalid plugin.version: {e}")))?;
    let _ = version;
    semver::VersionReq::parse(&out["sdk"]).map_err(|e| invalid(&format!("invalid plugin.sdk requirement: {e}")))?;
    Ok(out)
}

pub fn plugin_identifier(entries: &[Entry]) -> io::Result<String> {
    let fields = manifest_fields(&manifest(entries)?)?;
    Ok(fields["id"].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_package() {
        let id=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("snack-yummy-test-{id}"));
        let source=root.join("source"); let output=root.join("sample.yummy");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("manifest.toml"),
            "[plugin]\nid=\"local.sample\"\nname=\"Sample\"\nversion=\"0.1.0\"\nsdk=\"^1.0\"\nruntime=\"lua\"\n").unwrap();
        fs::write(source.join("main.lua"),b"print('hi')").unwrap();
        pack(&source,&output).unwrap();
        let entries=read(&output).unwrap();
        assert_eq!(entries.len(),2);
        assert_eq!(plugin_identifier(&entries).unwrap(),"local.sample");
        extract(&entries,&root.join("extracted")).unwrap();
        assert_eq!(fs::read(root.join("extracted/main.lua")).unwrap(),b"print('hi')");
        let _=fs::remove_dir_all(root);
    }
    #[test]
    fn rejects_path_traversal() {
        assert!(valid_name("../secret").is_err());
        assert!(valid_name("/etc/passwd").is_err());
        assert!(valid_name("ok/path.txt").is_ok());
        assert!(valid_name("folder/./file").is_err());
    }
}
