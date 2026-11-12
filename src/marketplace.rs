//! GitHub-based official Snack registry. Does not execute downloaded packages.
//! Central source: https://github.com/criteric/snack-plugins
use std::{env, fs, io::{self, Read, Write}, path::{Path, PathBuf}, process::{Command, Stdio}, time::{SystemTime, UNIX_EPOCH}};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use crate::package;

pub const OFFICIAL_REPO: &str = "criteric/snack-plugins";
pub const OFFICIAL_INDEX: &str = "https://raw.githubusercontent.com/criteric/snack-plugins/main/index.json";
fn failure(message: impl Into<String>) -> io::Error { io::Error::other(message.into()) }

pub fn hash_file(path: &Path) -> io::Result<String> {
    let mut input = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut chunk = [0u8; 65536];
    loop { let n = input.read(&mut chunk)?; if n == 0 { break; } hash.update(&chunk[..n]); }
    Ok(format!("{:x}", hash.finalize()))
}

fn parse_index(data: &[u8]) -> io::Result<Vec<Value>> {
    let value: Value = serde_json::from_slice(data).map_err(|e|failure(format!("invalid marketplace index JSON: {e}")))?;
    let items = if let Some(arr) = value.as_array() { arr }
        else if value["schema"] == 1 { value.get("plugins").and_then(Value::as_array).ok_or_else(||failure("marketplace index missing plugins array"))? }
        else { return Err(failure("unsupported marketplace index schema (expected 1)")); };
    if items.len() > 10000 { return Err(failure("marketplace index too large")); }
    for entry in items {
        for key in ["id", "version", "download_url", "sha256"] {
            if entry.get(key).and_then(Value::as_str).unwrap_or("").is_empty() {
                return Err(failure(format!("marketplace entry missing {key}")));
            }
        }
    }
    Ok(items.clone())
}

fn fetch_url(url: &str, dest: Option<&Path>) -> io::Result<Vec<u8>> {
    if !url.starts_with("https://") {return Err(failure("marketplace requires HTTPS"));}
    let mut command = Command::new("curl");
    command.args(["--fail", "--location", "--silent", "--show-error", "--max-time", "45", "--proto", "=https", "--proto-redir", "=https", "--user-agent", "snackctl/0.6.1"]);
    if let Some(path) = dest {command.args(["--max-filesize", "536870912", "--output"]).arg(path);}
    command.arg(url);
    let output = command.output().map_err(|e|failure(format!("curl is required: {e}")))?;
    if !output.status.success() {return Err(failure(format!("download failed for {url}: {}", String::from_utf8_lossy(&output.stderr).trim())));}
    Ok(output.stdout)
}

pub fn index() -> io::Result<Vec<Value>> {
    let target = env::var("SNACK_MARKETPLACE_INDEX").unwrap_or_else(|_| OFFICIAL_INDEX.to_owned());
    let data = if target.starts_with("https://") { fetch_url(&target, None)? } else {
        let p = Path::new(&target);
        if !p.is_file(){return Err(failure("SNACK_MARKETPLACE_INDEX must be an HTTPS URL or a readable local JSON file"));}
        fs::read(p)?
    };
    if data.len() > 4_000_000 {return Err(failure("marketplace index exceeds size limit"));}
    parse_index(&data)
}

pub fn search(term: &str) -> io::Result<usize> {
    let query = term.to_lowercase();
    let entries=index()?;
    let mut count=0;
    for entry in &entries {
        let id=entry["id"].as_str().unwrap_or("");
        let name=entry["name"].as_str().unwrap_or("");
        let description=entry["description"].as_str().unwrap_or("");
        if id.to_lowercase().contains(&query)||name.to_lowercase().contains(&query)||description.to_lowercase().contains(&query) {
            count+=1;
            println!("  [Snack Official] {id} v{} ({}) — {}",entry["version"].as_str().unwrap_or("?"),entry["runtime"].as_str().unwrap_or("?"),description);
        }
    }
    if count==0 {println!("  No official matches");}
    Ok(count)
}

fn valid_release_url(url: &str) -> bool {
    // Marketplace packages must be GitHub Release assets, not arbitrary executable URLs.
    url.starts_with("https://github.com/") && url.contains("/releases/download/") && url.ends_with(".yummy")
}

pub fn install_silent(id: &str, plugin_dir: &Path) -> io::Result<()> {
    download_install(id, plugin_dir, false)
}
pub fn update_silent(id: &str, plugin_dir: &Path) -> io::Result<()> {
    download_install(id, plugin_dir, true)
}
pub fn install(id: &str, plugin_dir: &Path) -> io::Result<()> {
    install_silent(id,plugin_dir)?;
    println!("Installed {id} from {OFFICIAL_REPO}. First execution requires permission confirmation.");
    Ok(())
}
fn download_install(id: &str, plugin_dir: &Path, replace: bool) -> io::Result<()> {
    // Download and verify BEFORE touching any existing installed package.
    if id.is_empty() || !id.bytes().all(|b|b.is_ascii_alphanumeric() || b"._-".contains(&b)) {
        return Err(failure("invalid plugin identifier"));
    }
    let entries = index()?;
    let item = entries.iter().find(|x| x["id"].as_str()==Some(id))
        .ok_or_else(||failure(format!("plugin {id:?} not found in official registry")))?;
    let url = item["download_url"].as_str().unwrap_or("");
    if !valid_release_url(url) {return Err(failure("registry package URL must be a GitHub Release .yummy asset"));}
    let expected_sha=item["sha256"].as_str().unwrap_or("");
    if expected_sha.len()!=64 || !expected_sha.bytes().all(|b|b.is_ascii_hexdigit()){
        return Err(failure("invalid registry SHA-256"));
    }
    fs::create_dir_all(plugin_dir)?;
    let target=plugin_dir.join(format!("{id}.yummy"));
    if !replace && target.exists() {return Err(failure(format!("{id} already installed")));}
    if replace && !target.exists(){return Err(failure(format!("{id} is not installed")));}
    let ts=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e|failure(e.to_string()))?.as_nanos();
    let temp=env::temp_dir().join(format!("snack-download-{}-{ts}.yummy",std::process::id()));
    let stage=plugin_dir.join(format!(".{id}-{ts}.staging"));
    let backup=plugin_dir.join(format!(".{id}-{ts}.backup"));
    let result=(|| -> io::Result<()> {
        fetch_url(url,Some(&temp))?;
        let actual=hash_file(&temp)?;
        if !expected_sha.eq_ignore_ascii_case(&actual) {return Err(failure("downloaded package SHA-256 does not match registry"));}
        let pkg=package::read(&temp)?;
        let fields=package::manifest_fields(&package::manifest(&pkg)?)?;
        if fields["id"]!=id || fields["version"]!=item["version"].as_str().unwrap_or("") {
            return Err(failure("downloaded package manifest differs from registry"));
        }
        if replace {
            let previous=package::read(&target)?;
            let old=package::manifest_fields(&package::manifest(&previous)?)?;
            let old_version=semver::Version::parse(&old["version"]).map_err(|e|failure(e.to_string()))?;
            let new_version=semver::Version::parse(&fields["version"]).map_err(|e|failure(e.to_string()))?;
            if new_version<=old_version{return Err(failure("no newer version available"));}
        }
        fs::copy(&temp,&stage)?;
        if replace {
            fs::rename(&target,&backup)?;
            if let Err(e)=fs::rename(&stage,&target){let _=fs::rename(&backup,&target);return Err(e);}
            let _=fs::remove_file(&backup);
        }else{
            if target.exists(){return Err(failure("plugin was installed concurrently"));}
            fs::rename(&stage,&target)?;
        }
        Ok(())
    })();
    let _=fs::remove_file(&temp);
    let _=fs::remove_file(&stage);
    result
}

fn gh_output(args:&[&str]) -> io::Result<Vec<u8>> {
    let output=Command::new("gh").args(args).output().map_err(|e|failure(format!("GitHub CLI unavailable; install gh and run gh auth login: {e}")))?;
    if !output.status.success() { return Err(failure(format!("gh {} failed: {}",args.join(" "), String::from_utf8_lossy(&output.stderr).trim()))); }
    Ok(output.stdout)
}

fn repo_valid(repo: &str) -> bool {
    let parts:Vec<_>=repo.split('/').collect();
    parts.len()==2 && parts.iter().all(|part|!part.is_empty() && part.chars().all(|c|c.is_ascii_alphanumeric()||c=='-'||c=='_'||c=='.'))
}

fn index_from_gh(repo: &str) -> io::Result<(Vec<Value>, Option<String>)> {
    let endpoint=format!("repos/{repo}/contents/index.json");
    let output=Command::new("gh").args(["api", &endpoint]).output().map_err(|e|failure(format!("GitHub CLI unavailable: {e}")))?;
    if !output.status.success() {
        let stderr=String::from_utf8_lossy(&output.stderr);
        if stderr.contains("404") { return Ok((Vec::new(),None)); }
        return Err(failure(format!("cannot retrieve index from GitHub: {stderr}")));
    }
    let obj:Value=serde_json::from_slice(&output.stdout).map_err(|e|failure(format!("invalid GitHub API response: {e}")))?;
    let sha=obj["sha"].as_str().ok_or_else(||failure("index.json response has no git SHA"))?.to_owned();
    let encoded=obj["content"].as_str().ok_or_else(||failure("index.json response has no base64 content"))?.replace('\n',"");
    let data=BASE64.decode(encoded.as_bytes()).map_err(|e|failure(format!("invalid index base64: {e}")))?;
    Ok((parse_index(&data)?,Some(sha)))
}

fn push_index(repo:&str, items:&[Value], old_sha: Option<&str>) -> io::Result<()> {
    let serialized=serde_json::to_vec_pretty(&json!({"schema":1,"plugins":items}))
        .map_err(|e|failure(e.to_string()))?;
    let mut body=json!({"message":"registry: publish Snack plugin", "content":BASE64.encode(serialized)});
    if let Some(sha)=old_sha {body["sha"]=Value::String(sha.to_owned());}
    let endpoint=format!("repos/{repo}/contents/index.json");
    let mut child=Command::new("gh").args(["api","-X","PUT",&endpoint,"--input","-"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|e|failure(format!("GitHub CLI unavailable: {e}")))?;
    if let Some(mut input)=child.stdin.take(){input.write_all(body.to_string().as_bytes())?;}
    let out=child.wait_with_output()?;
    if !out.status.success(){return Err(failure(format!("GitHub index update failed: {}",String::from_utf8_lossy(&out.stderr))));}
    Ok(())
}

fn description_from_manifest(source: &str) -> String {
    source.parse::<toml::Value>().ok()
        .and_then(|v|v.get("plugin")?.get("description")?.as_str().map(str::to_owned))
        .unwrap_or_default()
}

pub fn publish(path:&Path, repo:&str, dry_run:bool) -> io::Result<()> {
    if !repo_valid(repo) {return Err(failure("repository must be formatted as owner/name"));}
    let pkg=package::read(path)?;
    let manifest=package::manifest(&pkg)?;
    let fields=package::manifest_fields(&manifest)?;
    let version=&fields["version"];
    let tag=format!("plugin-{}-v{}",fields["id"],version);
    let filename=path.file_name().ok_or_else(||failure("package has no filename"))?.to_string_lossy();
    let url=format!("https://github.com/{repo}/releases/download/{tag}/{filename}");
    let checksum=hash_file(path)?;
    let mut entry=json!({
        "id":fields["id"],"name":fields["name"],"version":version,
        "sdk":fields["sdk"],"runtime":fields["runtime"],
        "description":description_from_manifest(&manifest),
        "publisher":repo.split('/').next().unwrap_or(""),
        "download_url":url,"sha256":checksum
    });
    if let Some(lang)=fields.get("language"){entry["language"]=Value::String(lang.clone());}
    println!("Package: {}\nRepository: {repo}\nRelease tag: {tag}\nSHA-256: {checksum}",path.display());
    if dry_run { println!("Dry run only; no GitHub changes.\nRegistry entry: {}",serde_json::to_string_pretty(&entry).unwrap_or_default());return Ok(()); }
    let official=repo==OFFICIAL_REPO;
    let (mut items,old_sha)=if official {index_from_gh(repo)?} else {(Vec::new(),None)};
    if official {
        if let Some(old)=items.iter().find(|x|x["id"].as_str()==Some(fields["id"].as_str())) {
            if let Some(old_ver)=old["version"].as_str() {
                let current=semver::Version::parse(old_ver).map_err(|e|failure(format!("invalid existing version: {e}")))?;
                let next=semver::Version::parse(version).map_err(|e|failure(e.to_string()))?;
                if next <= current {return Err(failure(format!("marketplace already lists {} v{}; publish a newer version",fields["id"],old_ver)));}
            }
        }
    }
    let title=format!("{} v{}",fields["name"],version);
    let notes=format!("Snack .yummy package: {} v{}",fields["id"],version);
    gh_output(&["release","create",&tag,&path.to_string_lossy(),"--repo",repo,"--title",&title,"--notes",&notes,"--latest=false"])?;
    println!("Release asset published: {url}");
    if official {
        items.retain(|item|item["id"].as_str()!=Some(fields["id"].as_str()));
        items.push(entry);
        items.sort_by(|a,b|a["id"].as_str().cmp(&b["id"].as_str()));
        push_index(repo,&items,old_sha.as_deref())?;
        println!("Official registry index.json updated. Search: snackctl search {}",fields["id"]);
    } else {
        println!("This package was published to an external repository only. Submit a PR to {OFFICIAL_REPO} to list it in the official marketplace.");
    }
    Ok(())
}

pub fn notify_updates(plugin_dir:&Path) -> io::Result<()> {
    let items=index()?;let mut count=0;
    if !plugin_dir.exists(){println!("No installed plugins");return Ok(());}
    for item in fs::read_dir(plugin_dir)? {
        let path=item?.path();if path.extension().and_then(|s|s.to_str())!=Some("yummy"){continue;}
        let Ok(entries)=package::read(&path) else{continue};
        let Ok(f)=package::manifest_fields(&package::manifest(&entries)?) else{continue};
        if let Some(remote)=items.iter().find(|v|v["id"].as_str()==Some(f["id"].as_str())) {
            if let Some(v)=remote["version"].as_str(){
                if let (Ok(current),Ok(new))=(semver::Version::parse(&f["version"]),semver::Version::parse(v)) {
                    if new>current {count+=1;println!("Update available: {} {} -> {} (install only after your approval)",f["id"],current,new);}
                }
            }
        }
    }
    if count==0 {println!("Installed official plugins are up to date (or no official matches)");}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn parses_empty_v1(){assert!(parse_index(br#"{"schema":1,"plugins":[]}"#).unwrap().is_empty());}
    #[test] fn refuses_bad_schema(){assert!(parse_index(br#"{"schema":2,"plugins":[]}"#).is_err());}
    #[test] fn validates_release_url(){assert!(valid_release_url("https://github.com/criteric/snack-plugins/releases/download/plugin-a-v1/a.yummy"));assert!(!valid_release_url("http://evil.net/plugin.yummy"));}
    #[test] fn validates_repo(){assert!(repo_valid("criteric/snack-plugins"));assert!(!repo_valid("owner/repo/extra"));}
}
