//! Expandable Explorer tree. Directories are lazy: children are read only when opened.
use std::{collections::{HashMap,HashSet}, fs, path::{Path,PathBuf}};

fn visible(path:&Path)->bool {
    path.file_name().and_then(|n|n.to_str()).map(|n| !n.starts_with('.') || n==".gitignore").unwrap_or(false)
}
pub fn refresh(root:&Path, expanded:&HashSet<PathBuf>) -> (Vec<PathBuf>,HashMap<PathBuf,usize>) {
    let mut tree=Vec::new();let mut depths=HashMap::new();
    if let Some(parent)=root.parent() {tree.push(parent.to_path_buf());depths.insert(parent.to_path_buf(),0);}
    descend(root,0,expanded,&mut tree,&mut depths);
    (tree,depths)
}
fn descend(dir:&Path,depth:usize,expanded:&HashSet<PathBuf>,tree:&mut Vec<PathBuf>,depths:&mut HashMap<PathBuf,usize>) {
    if depth>10 || tree.len()>5000{return;}
    let Ok(entries)=fs::read_dir(dir) else{return;};
    let mut items:Vec<PathBuf>=entries.flatten().map(|e|e.path()).filter(|p|visible(p)).collect();
    items.sort_by_key(|p|(!p.is_dir(),p.file_name().map(|s|s.to_os_string())));
    for path in items {
        depths.insert(path.clone(),depth);
        tree.push(path.clone());
        if path.is_dir() && !path.is_symlink() && expanded.contains(&path){
            descend(&path,depth+1,expanded,tree,depths);
        }
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn root_listing(){let mut x=HashSet::new();let t=refresh(&std::env::temp_dir(),&x).0;assert!(!t.is_empty());x.clear();}
}
