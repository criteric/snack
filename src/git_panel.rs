//! Git status parsing; no automatic modifications to the index or working tree.
use std::{path::{Path,PathBuf},process::Command};
#[derive(Clone)] pub struct Change {pub state:String,pub path:PathBuf}
pub fn changes(root:&Path)->Vec<Change>{
    let Ok(out)=Command::new("git").args(["status","--porcelain=v1","--untracked-files=normal"]).current_dir(root).output() else{return vec![]};
    if !out.status.success(){return vec![];}
    String::from_utf8_lossy(&out.stdout).lines().filter_map(|line|{
        if line.len()<4{return None;}
        let state=line.get(0..2)?.to_owned();let name=line.get(3..)?;
        let p=if let Some((_,renamed))=name.rsplit_once(" -> "){renamed}else{name};
        Some(Change{state,path:root.join(p.trim_matches('"'))})
    }).collect()
}
