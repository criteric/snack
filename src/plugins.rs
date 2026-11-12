//! Installed .yummy packages and explicit-on-demand Lua plugin execution.
//! A Lua plugin is never executed on installation or startup.
use std::{env,fs,io::{self,Write},path::{Path,PathBuf},process::{Command,Stdio},time::{SystemTime,UNIX_EPOCH}};
use sha2::{Sha256,Digest};
use crate::package;
#[derive(Clone)]
pub struct Plugin {pub id:String,pub name:String,pub version:String,pub runtime:String,pub archive:PathBuf,pub greeting:String}
pub enum Action {Message(String),Insert(String)}
pub fn directory()->PathBuf{
    if cfg!(windows){env::var_os("APPDATA").or_else(||env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default().join("snack/plugins")}
    else{env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(||PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".local/share")).join("snack/plugins")}
}
pub fn load()->Vec<Plugin>{
    let mut result=Vec::new();let Ok(items)=fs::read_dir(directory())else{return result};
    for item in items.flatten(){let path=item.path();if path.extension().and_then(|e|e.to_str())!=Some("yummy"){continue;}
        let Ok(entries)=package::read(&path)else{continue};let Ok(manifest)=package::manifest(&entries)else{continue};
        let Ok(f)=package::manifest_fields(&manifest)else{continue};
        result.push(Plugin{id:f["id"].clone(),name:f["name"].clone(),version:f["version"].clone(),runtime:f["runtime"].clone(),archive:path,greeting:String::new()});
    }
    result
}
fn trust_store()->PathBuf {crate::config::Config::path().with_file_name("trusted-plugins.txt")}
fn fingerprint(plugin:&Plugin)->io::Result<String>{
    let bytes=fs::read(&plugin.archive)?;
    Ok(format!("{}:{}:{:x}",plugin.id,plugin.version,Sha256::digest(bytes)))
}
pub fn trusted(plugin:&Plugin)->bool {
    let Ok(id)=fingerprint(plugin)else{return false};
    fs::read_to_string(trust_store()).map(|x|x.lines().any(|line|line==id)).unwrap_or(false)
}
pub fn trust(plugin:&Plugin)->io::Result<()>{
    let store=trust_store();if let Some(parent)=store.parent(){fs::create_dir_all(parent)?;}
    let mut f=fs::OpenOptions::new().create(true).append(true).open(store)?;
    writeln!(f,"{}",fingerprint(plugin)?)
}
pub fn run(plugin:&Plugin)->io::Result<Vec<Action>> {
    if plugin.runtime!="lua"{return Err(io::Error::other("Only Lua .yummy execution is supported in Snack 0.6; native/WASM runtimes are not enabled"));}
    let entries=package::read(&plugin.archive)?;
    if !entries.iter().any(|e|e.name=="src/main.lua") {return Err(io::Error::other("Lua package missing src/main.lua"));}
    let nanos=SystemTime::now().duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_nanos();
    let temp=env::temp_dir().join(format!("snack-plugin-{}-{nanos}",std::process::id()));
    package::extract(&entries,&temp)?;
    let interpreter=if Command::new("lua").arg("-v").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok(){"lua"}else{"lua5.4"};
    let output=Command::new(interpreter).arg("src/main.lua").current_dir(&temp).output();
    let _=fs::remove_dir_all(&temp);
    let output=output.map_err(|e|io::Error::other(format!("Lua runtime missing (install lua5.4): {e}")))?;
    if !output.status.success(){return Err(io::Error::other(format!("Lua plugin error: {}",String::from_utf8_lossy(&output.stderr))));}
    let mut actions=Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines().take(100){
        if let Some(msg)=line.strip_prefix("SNACK_MESSAGE\t"){actions.push(Action::Message(msg.to_string()));}
        else if let Some(text)=line.strip_prefix("SNACK_INSERT\t"){actions.push(Action::Insert(text.to_string()));}
    }
    Ok(actions)
}
#[cfg(test)] mod tests {use super::*;#[test]fn missing_plugins_ok(){let _=load();}}
