use std::{env, fs, io, path::PathBuf};

#[derive(Clone)]
pub struct Config {
    pub autosave_seconds: u64,
    pub show_tree: bool,
    pub vim_mode: bool,
    pub tab_width: usize,
    pub autocomplete: bool,
}
impl Default for Config {
    fn default() -> Self { Self { autosave_seconds: 30, show_tree: true, vim_mode: false, tab_width: 4, autocomplete: true } }
}
impl Config {
    pub fn path() -> PathBuf {
        let base=if cfg!(windows) {
            env::var_os("APPDATA").or_else(||env::var_os("USERPROFILE")).map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
        } else {
            env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"))
        };
        base.join("snack/config.toml")
    }
    pub fn load() -> Self {
        let mut c = Self::default();
        if let Ok(input) = fs::read_to_string(Self::path()) {
            for line in input.lines() {
                let line=line.split('#').next().unwrap_or("").trim();
                let Some((k,v))=line.split_once('=') else {continue};
                let (k,v)=(k.trim(),v.trim().trim_matches('"'));
                let _=c.set(k,v);
            }
        }
        c
    }
    pub fn set(&mut self, key:&str, value:&str)->Result<(),String>{
        match key {
            "autosave_seconds"=>self.autosave_seconds=value.parse::<u64>().map_err(|_|"expected nonnegative integer".to_string())?.min(3600),
            "autocomplete"=>self.autocomplete=value.parse::<bool>().map_err(|_|"expected true or false".to_string())?,
            "show_tree"=>self.show_tree=value.parse::<bool>().map_err(|_|"expected true or false".to_string())?,
            "vim_mode"=>self.vim_mode=value.parse::<bool>().map_err(|_|"expected true or false".to_string())?,
            "tab_width"=>self.tab_width=value.parse::<usize>().map_err(|_|"expected integer".to_string())?.clamp(1,8),
            _=>return Err(format!("unknown setting: {key}")),
        }
        Ok(())
    }
    pub fn get(&self,key:&str)->String{
        match key {
            "autosave_seconds"=>self.autosave_seconds.to_string(),
            "autocomplete"=>self.autocomplete.to_string(),
            "show_tree"=>self.show_tree.to_string(),
            "vim_mode"=>self.vim_mode.to_string(),
            "tab_width"=>self.tab_width.to_string(),
            _=>"".into(),
        }
    }
    pub fn save(&self)->io::Result<()> {
        let path=Self::path();if let Some(parent)=path.parent(){fs::create_dir_all(parent)?;}
        fs::write(path,format!("# Snack settings\nautosave_seconds = {}\nshow_tree = {}\nvim_mode = {}\ntab_width = {}\nautocomplete = {}\n",self.autosave_seconds,self.show_tree,self.vim_mode,self.tab_width,self.autocomplete))
    }
}
