mod clipboard;
mod completion;
mod emmet;
mod explorer;
mod git_panel;
mod lsp;
mod terminal_panel;
mod platform;
mod config;
mod document;
mod plugins;
mod marketplace;
mod marketplace_ui;
mod package;
mod settings;

use std::{collections::{HashMap,HashSet}, env, fs, io::{self, Stdout, Write}, path::{Path, PathBuf}, process::Command, time::{Duration, Instant}};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, PushKeyboardEnhancementFlags, PopKeyboardEnhancementFlags, MouseButton, MouseEventKind},
    execute, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use config::Config;
use document::Document;
use completion::Candidate;
use platform::Picker;

const BG:Color=Color::Rgb{r:26,g:27,b:38};
const PANEL:Color=Color::Rgb{r:36,g:40,b:59};
const TEXT:Color=Color::Rgb{r:192,g:202,b:245};
const MUTED:Color=Color::Rgb{r:105,g:112,b:151};
const BLUE:Color=Color::Rgb{r:122,g:162,b:247};
const GREEN:Color=Color::Rgb{r:158,g:206,b:106};
const PURPLE:Color=Color::Rgb{r:187,g:154,b:247};
const YELLOW:Color=Color::Rgb{r:224,g:175,b:104};
const RED:Color=Color::Rgb{r:247,g:118,b:142};

#[derive(Clone,Copy,PartialEq,Eq)]
enum Menu {File, View, Edit, Help, Extensions}
impl Menu {
    fn items(self)-> &'static [&'static str] {match self {
        Self::File => &["New Text File", "New File in Folder...", "New Folder...", "Open File...", "Open Folder...", "Open Recent", "Save", "Save As...", "Save All", "Auto Save", "Close Editor", "Close Folder", "Exit"],
        Self::View => &["Toggle File Tree", "Git Changes", "Refresh Git", "Refresh Files", "Next Tab", "Previous Tab", "Toggle Vim Mode", "Open Shell", "Terminal Panel", "Problems", "Extensions Marketplace...", "Run Plugin...", "Settings..."],
        Self::Edit => &["Undo", "Redo", "Cut", "Copy", "Paste", "Select All", "Select Line", "Duplicate Line", "Delete Line", "Comment/Uncomment", "Find...", "Replace...", "Delete Character"],
        Self::Help => &["Keyboard shortcuts", "About Snack"],
        Self::Extensions => &["Marketplace...", "Installed Plugins", "Refresh Marketplace", "Run Plugin..."],
    }}
    fn title(self)-> &'static str {match self {Self::File=>"File",Self::View=>"View",Self::Edit=>"Edit",Self::Help=>"Help",Self::Extensions=>"Extensions"}}
}
#[derive(Clone)]
enum Prompt {None, Find(String), Open(String), NewFile(String), NewFolder(String), Rename(String), SaveAs(String), DeleteConfirm(String), ReplaceFrom(String), ReplaceTo(String,String), RunPlugin(String), PluginConsent(String) }
impl Prompt {
    fn text_mut(&mut self)->Option<&mut String>{match self {Self::None=>None,Self::Find(s)|Self::Open(s)|Self::NewFile(s)|Self::NewFolder(s)|Self::Rename(s)|Self::SaveAs(s)|Self::DeleteConfirm(s)|Self::ReplaceFrom(s)|Self::ReplaceTo(_,s)|Self::RunPlugin(s)|Self::PluginConsent(s)=>Some(s)}}
    fn label(&self)->&'static str{match self {Self::None=>"",Self::Find(_)=>"Find",Self::Open(_)=>"Open",Self::NewFile(_)=>"New file name",Self::NewFolder(_)=>"New folder name",Self::Rename(_)=>"Rename to",Self::SaveAs(_)=>"Save as",Self::DeleteConfirm(_)=>"Type DELETE to confirm",Self::ReplaceFrom(_)=>"Find text",Self::ReplaceTo(_,_)=>"Replace with",Self::RunPlugin(_)=>"Plugin ID",Self::PluginConsent(_)=>"Type YES to allow this plugin"}}
    fn active(&self)->bool{!matches!(self,Self::None)}
}
#[derive(Clone)]
enum ContextTarget {
    Explorer(Option<PathBuf>), Git(Option<PathBuf>), Editor, Tab(usize), TabBar,
    Terminal, Problems, Status,
}
struct ContextMenu {
    x:u16, y:u16, target:ContextTarget, width:u16, height:usize, offset:usize,
    items:Vec<&'static str>,
}
fn context_items(target:&ContextTarget)->&'static [&'static str] {
    match target {
        ContextTarget::Explorer(Some(path)) if path.is_dir()=> &[
            "Open Folder", "Expand/Collapse", "New File Here", "New Folder Here",
            "Copy Path", "Copy Name", "Rename selected", "Delete selected",
            "Reveal in Files", "Refresh Files", "Open Folder...",
        ],
        ContextTarget::Explorer(Some(_))=> &[
            "Open", "New File Here", "New Folder Here", "Copy Path", "Copy Name",
            "Rename selected", "Delete selected", "Reveal in Files", "Refresh Files",
        ],
        ContextTarget::Explorer(None)=> &[
            "New File Here", "New Folder Here", "Open Folder...", "Refresh Files",
            "Git Changes", "Toggle File Tree",
        ],
        ContextTarget::Git(Some(_))=> &[
            "Open", "Copy Path", "Reveal in Files", "Refresh Git", "Show Explorer",
        ],
        ContextTarget::Git(None)=> &["Refresh Git", "Show Explorer"],
        ContextTarget::Editor=> &[
            "Cut", "Copy", "Paste", "Select All", "Select Line", "Duplicate Line",
            "Delete Line", "Comment/Uncomment", "Undo", "Redo", "Find...",
            "Replace...", "Save", "Copy File Path", "Reveal in Files",
        ],
        ContextTarget::Tab(_)=> &[
            "Save", "Save As...", "Copy File Path", "Reveal in Files",
            "Close Tab", "Close Other Tabs", "Close Tabs to Right", "New Text File", "Open File...",
        ],
        ContextTarget::TabBar=> &["New Text File", "Open File...", "Open Folder...", "Save All"],
        ContextTarget::Terminal=> &[
            "Copy Terminal Output", "Copy Command", "Paste Command", "Clear Terminal",
            "Run Command", "Focus Terminal", "Hide Terminal", "Open Shell",
        ],
        ContextTarget::Problems=> &[
            "Copy Diagnostics", "Clear Diagnostics", "Request LSP Completion", "Hide Problems",
            "Copy File Path",
        ],
        ContextTarget::Status=> &[
            "Toggle File Tree", "Git Changes", "Terminal Panel", "Problems",
            "Settings...", "Copy File Path", "About Snack",
        ],
    }
}
impl ContextMenu {
    fn new(target:ContextTarget,x:u16,y:u16,w:u16,h:u16)->Self {
        let items=context_items(&target).to_vec();
        let width=(items.iter().map(|s|s.chars().count()).max().unwrap_or(16)+3).max(20)
            .min(w.saturating_sub(1).max(1) as usize) as u16;
        let height=items.len().min(h.saturating_sub(3).max(1) as usize).min(12);
        let x=x.min(w.saturating_sub(width));
        let y=y.min(h.saturating_sub(2+height as u16));
        Self{x,y,target,width,height,offset:0,items}
    }
    fn contains(&self,x:u16,y:u16)->bool {
        x>=self.x && x<self.x+self.width && y>=self.y && y<self.y+self.height as u16
    }
}

struct App {
    docs:Vec<Document>, active:usize, cwd:PathBuf, config:Config, plugins:Vec<plugins::Plugin>, market:marketplace_ui::Browser,
    tree:Vec<PathBuf>, tree_depth:HashMap<PathBuf,usize>, expanded_dirs:HashSet<PathBuf>, sidebar_git:bool, git_changes:Vec<git_panel::Change>, tree_offset:usize, selected:Option<PathBuf>, menu:Option<Menu>, context:Option<ContextMenu>,
    prompt:Prompt, message:String, vim_normal:bool, welcome:bool, dashboard:bool, dashboard_start:Instant, dashboard_tick:usize, dashboard_selected:usize, quit_armed:bool,
    completion:Vec<Candidate>, completion_selected:usize, lsp:lsp::Client, diagnostics:Vec<String>,
    terminal:terminal_panel::Panel, plugin_pending:Option<plugins::Plugin>, problems_visible:bool,
    git:String, needs_draw:bool, shell_requested:bool, settings_requested:bool, picker_requested:Option<Picker>, prev:Option<Frame>, recent:Vec<PathBuf>, auto_save:bool, untitled_seq:usize, clipboard:String, clipboard_local_only:bool, mouse_anchor:Option<(usize,usize)>,
}
impl App {
    fn new(paths:Vec<PathBuf>)->io::Result<Self>{
        let initial=env::current_dir()?;
        let launched_empty=paths.is_empty();
        let mut cwd=if launched_empty {env::var_os("HOME").map(PathBuf::from).filter(|p|p.is_dir()).unwrap_or_else(||initial.clone())} else {initial.clone()};
        let mut docs=Vec::new();
        for p in paths {
            let absolute=if p.is_absolute(){p}else{initial.join(p)};
            if absolute.is_dir(){cwd=absolute;continue;}
            if let Some(parent)=absolute.parent(){if parent.is_dir(){cwd=parent.to_path_buf();}}
            docs.push(Document::open(absolute)?);
        }
        let welcome=docs.is_empty();
        if docs.is_empty(){docs.push(Document::open(cwd.join("untitled.txt"))?);}
        let config=Config::load();
        let plugins=plugins::load();
        let git=Command::new("git").args(["branch","--show-current"]).current_dir(&cwd).output().ok()
            .filter(|o|o.status.success()).map(|o|String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        let mut a=Self{docs,active:0,cwd,config,plugins,market:marketplace_ui::Browser::new(),tree:vec![],tree_depth:HashMap::new(),expanded_dirs:HashSet::new(),sidebar_git:false,git_changes:vec![],tree_offset:0,selected:None,
            menu:None,context:None,prompt:Prompt::None,message:"Snack 0.6.4 | F9 Extensions Marketplace | Ctrl+Space LSP".into(),
            vim_normal:false,welcome,dashboard:launched_empty,dashboard_start:Instant::now(),dashboard_tick:0,dashboard_selected:0,quit_armed:false,completion:vec![],completion_selected:0,lsp:lsp::Client::new(),diagnostics:vec![],terminal:terminal_panel::Panel::new(),plugin_pending:None,problems_visible:false,git,needs_draw:true,shell_requested:false,settings_requested:false,picker_requested:None,prev:None,recent:vec![],auto_save:false,untitled_seq:1,clipboard:String::new(),clipboard_local_only:false,mouse_anchor:None};
        a.vim_normal=a.config.vim_mode;
        if launched_empty {a.config.show_tree=false;}
        a.refresh_tree();a.refresh_git();
        if let Some(p)=a.plugins.iter().find(|p|!p.greeting.is_empty()){a.message=p.greeting.clone();}
        Ok(a)
    }
    fn update_git(&mut self) {
        self.git=Command::new("git").args(["branch","--show-current"]).current_dir(&self.cwd)
            .output().ok().filter(|o|o.status.success())
            .map(|o|String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    }
    fn doc(&self)->&Document{&self.docs[self.active]}
    fn doc_mut(&mut self)->&mut Document{&mut self.docs[self.active]}
    fn copy_text(&mut self, text:String){
        self.clipboard=text.clone();
        match clipboard::write(&text){
            Ok(())=>{self.clipboard_local_only=false;self.message="Copied to system clipboard".into();},
            Err(_)=>{self.clipboard_local_only=true;self.message="Copied inside Snack (install xclip or wl-clipboard for system clipboard)".into();},
        }
    }
    fn paste_text(&mut self){
        let text=if self.clipboard_local_only {self.clipboard.clone()} else {
            clipboard::read().ok().filter(|s|!s.is_empty()).unwrap_or_else(||self.clipboard.clone())
        };
        if text.is_empty(){self.message="Clipboard is empty".into();return;}
        // Normalize CRLF and reject terminal control codes except tabs and line breaks.
        let normalized=text.replace("\r\n","\n").replace('\r',"\n");
        let cleaned:String=normalized.chars().filter(|c|!c.is_control()||*c=='\n'||*c=='\t').take(2_000_000).collect();
        self.doc_mut().replace_selection(&cleaned);
        self.welcome=false;self.completion.clear();self.message="Pasted".into();
    }
    fn cut_text(&mut self){
        let text=self.doc().copy_selection_or_line();
        self.copy_text(text);
        if !self.doc_mut().delete_selection(){self.doc_mut().delete_line();}
        self.message="Cut".into();
    }
    fn reveal_path(&mut self,path:PathBuf){
        let target=if path.is_dir(){path}else{path.parent().map(Path::to_path_buf).unwrap_or(self.cwd.clone())};
        let result=if cfg!(target_os="windows"){
            Command::new("explorer.exe").arg(&target).spawn()
        }else if cfg!(target_os="macos"){
            Command::new("open").arg(&target).spawn()
        }else{
            Command::new("xdg-open").arg(&target).spawn()
        };
        match result{Ok(_)=>self.message="Opened in file manager".into(),Err(e)=>self.message=format!("File manager error: {e}")}
    }
    fn tab_at(&self,col:u16,side:u16)->Option<usize>{
        if col<side{return None;}
        let mut x=side;
        for (i,d) in self.docs.iter().enumerate(){
            let width=d.title().chars().count() as u16+5;
            if col>=x && col<x.saturating_add(width){return Some(i);}
            x=x.saturating_add(width);
        }
        None
    }
    fn context_target_at(&self,col:u16,row:u16,w:u16,h:u16)->ContextTarget{
        let side=self.sidebar_width(w);
        if row>=h.saturating_sub(2){return ContextTarget::Status;}
        if row==1 && col>=side{return self.tab_at(col,side).map_or(ContextTarget::TabBar,ContextTarget::Tab);}
        // Bottom panel visually overlays the sidebar and editor: it wins hit-testing.
        if self.terminal.visible && row>=h.saturating_sub(10){return ContextTarget::Terminal;}
        if self.problems_visible && row>=h.saturating_sub(10){return ContextTarget::Problems;}
        if side>0 && col<side {
            if row>=3 {
                let idx=self.tree_offset+(row-3) as usize;
                if self.sidebar_git {return ContextTarget::Git(self.git_changes.get(idx).map(|c|c.path.clone()));}
                let item=self.tree.get(idx).cloned().filter(|p|p.as_path()!=self.cwd.parent().unwrap_or(Path::new("")));
                return ContextTarget::Explorer(item);
            }
            return if self.sidebar_git{ContextTarget::Git(None)}else{ContextTarget::Explorer(None)};
        }
        if row>=3 {ContextTarget::Editor}else{ContextTarget::Status}
    }
    fn context_action(&mut self,target:ContextTarget,action:&str){
        // Context menu operations are anchored to the clicked item, not stale Explorer selection.
        match &target {
            ContextTarget::Explorer(path)=>self.selected=path.clone(),
            ContextTarget::Git(path)=>self.selected=path.clone(),
            ContextTarget::Tab(i)=>{if *i<self.docs.len(){self.active=*i;}},
            _=>{},
        }
        match action {
            "Copy"=>self.copy_text(self.doc().copy_selection_or_line()),
            "Cut"=>self.cut_text(),
            "Paste"=>self.paste_text(),
            "Select All"=>self.doc_mut().select_all(),
            "Select Line"=>self.doc_mut().select_line(),
            "Duplicate Line"=>self.doc_mut().duplicate_line(),
            "Delete Line"=>self.doc_mut().delete_line(),
            "Comment/Uncomment"=>self.doc_mut().toggle_line_comment(),
            "Copy Path"|"Copy File Path"=>{
                let path=match &target{
                    ContextTarget::Explorer(Some(p))|ContextTarget::Git(Some(p))=>p.clone(),
                    _=>self.doc().path.clone(),
                };
                self.copy_text(path.to_string_lossy().into_owned());
            }
            "Copy Name"=>{
                if let ContextTarget::Explorer(Some(path))=&target {
                    self.copy_text(path.file_name().unwrap_or_default().to_string_lossy().to_string());
                }
            }
            "Reveal in Files"=>{
                let path=match &target{
                    ContextTarget::Explorer(Some(p))|ContextTarget::Git(Some(p))=>p.clone(),
                    _=>self.doc().path.clone(),
                };
                self.reveal_path(path);
            }
            "Open"|"Open Folder"=>{
                match &target {
                    ContextTarget::Explorer(Some(p))|ContextTarget::Git(Some(p))=>self.open_path(p.clone()),
                    _=>{},
                }
            }
            "Expand/Collapse"=>{
                if let ContextTarget::Explorer(Some(p))=target {self.toggle_directory(p);}
            }
            "New File Here"=>self.prompt=Prompt::NewFile(String::new()),
            "New Folder Here"=>self.prompt=Prompt::NewFolder(String::new()),
            "Show Explorer"=>{self.sidebar_git=false;self.tree_offset=0;self.config.show_tree=true;self.refresh_tree();},
            "Close Other Tabs"=>{
                if self.docs.iter().enumerate().any(|(i,d)|i!=self.active&&d.dirty){
                    self.message="Save modified tabs before closing other tabs".into();
                }else{let active=self.docs.remove(self.active);self.docs.clear();self.docs.push(active);self.active=0;self.message="Closed other tabs".into();}
            }
            "Close Tabs to Right"=>{
                if self.docs.iter().skip(self.active+1).any(|d|d.dirty){
                    self.message="Save modified tabs on the right before closing".into();
                }else{self.docs.truncate(self.active+1);self.message="Closed tabs to the right".into();}
            }
            "Copy Terminal Output"=>self.copy_text(self.terminal.lines.join("\n")),
            "Copy Command"=>self.copy_text(self.terminal.input.clone()),
            "Paste Command"=>{
                let text=if self.clipboard_local_only {self.clipboard.clone()} else {
            clipboard::read().ok().filter(|s|!s.is_empty()).unwrap_or_else(||self.clipboard.clone())
        };
                self.terminal.input.push_str(&text.replace('\n'," ").replace('\r'," "));
                self.terminal.focus=true;
            }
            "Clear Terminal"=>{self.terminal.lines.clear();self.message="Terminal output cleared".into();},
            "Run Command"=>{self.terminal.execute(self.cwd.clone());},
            "Focus Terminal"=>{self.terminal.visible=true;self.terminal.focus=true;},
            "Hide Terminal"=>{self.terminal.visible=false;self.terminal.focus=false;},
            "Copy Diagnostics"=>self.copy_text(self.diagnostics.join("\n")),
            "Clear Diagnostics"=>{self.diagnostics.clear();self.message="Diagnostics cleared".into();},
            "Request LSP Completion"=>self.request_lsp(),
            "Hide Problems"=>self.problems_visible=false,
            _=>self.do_action(action),
        }
        self.completion.clear();self.needs_draw=true;
    }
    fn refresh_completion(&mut self,force:bool){
        self.completion=if self.config.autocomplete && !self.prompt.active() && !self.vim_normal {
            completion::suggestions(self.doc(),force)
        } else {vec![]};
        self.completion_selected=0;
    }
    fn request_lsp(&mut self){
        let d=self.doc();let request=lsp::Request{
            path:d.path.clone(),root:self.cwd.clone(),text:d.text(),line:d.y,
            column:d.lines[d.y].iter().take(d.x).map(|c|c.len_utf16()).sum(),
            prefix:completion::typed_prefix(d),
        };
        if self.lsp.sender.send(request).is_ok(){self.message="Requesting semantic LSP completion...".into();}
    }
    fn poll_lsp(&mut self){
        while let Ok(reply)=self.lsp.results.try_recv(){
            match reply {
                lsp::Reply::Completion{path,items}=>{
                    if path!=self.doc().path{continue;}
                    let prefix=completion::typed_prefix(self.doc());
                    let replace=prefix.chars().count();
                    let mut existing:std::collections::HashSet<String>=self.completion.iter().map(|c|c.label.clone()).collect();
                    for it in items.into_iter().take(32){
                        if !existing.insert(it.label.clone()){continue;}
                        let offset=it.insert.chars().count();
                        self.completion.push(Candidate{label:it.label,description:it.detail,replacement:it.insert,cursor_offset:offset,replace_chars:replace});
                    }
                    self.completion.truncate(64);self.needs_draw=true;
                }
                lsp::Reply::Diagnostics{path,items}=>{if path==self.doc().path{self.diagnostics=items;self.needs_draw=true;}},
                lsp::Reply::Message(text)=>{self.message=text;self.needs_draw=true;}
            }
        }
    }
    fn apply_completion(&mut self){
        let Some(choice)=self.completion.get(self.completion_selected).cloned() else{return;};
        self.doc_mut().replace_before_cursor(choice.replace_chars,&choice.replacement,choice.cursor_offset);
        self.completion.clear();
        self.message=format!("Completed: {} ({})",choice.label,choice.description);
        self.needs_draw=true;
    }
    fn refresh_tree(&mut self){
        let (items,depths)=explorer::refresh(&self.cwd,&self.expanded_dirs);
        self.tree=items;self.tree_depth=depths;
        if self.tree_offset>=self.tree.len(){self.tree_offset=0;}
        self.needs_draw=true;
    }
    fn run_plugin(&mut self,id:String){
        self.plugins=plugins::load();
        let id=id.trim();
        let selected=if id.is_empty() && self.plugins.len()==1 {self.plugins.first().cloned()}
            else{self.plugins.iter().find(|p|p.id==id).cloned()};
        let Some(plugin)=selected else{self.message=format!("Plugin not found. Installed: {}",self.plugins.iter().map(|p|p.id.as_str()).collect::<Vec<_>>().join(", "));return;};
        if !plugins::trusted(&plugin){self.message=format!("{} requests unrestricted Lua execution; type YES to allow this exact package",plugin.name);
            self.plugin_pending=Some(plugin);self.prompt=Prompt::PluginConsent(String::new());return;
        }
        self.execute_plugin(plugin);
    }
    fn execute_plugin(&mut self,plugin:plugins::Plugin){
        match plugins::run(&plugin){
            Ok(actions)=>{let mut count=0;
                for a in actions {match a{plugins::Action::Message(s)=>self.message=s,
                    plugins::Action::Insert(s)=>{for c in s.chars(){if c=='\n'{self.doc_mut().newline();}else{self.doc_mut().insert(c);}}count+=1;}}}
                if count>0{self.message=format!("Plugin {} inserted text",plugin.id);}
            },Err(e)=>self.message=format!("Plugin error: {e}")
        }
        self.needs_draw=true;
    }
    fn refresh_git(&mut self){self.update_git();self.git_changes=git_panel::changes(&self.cwd);self.needs_draw=true;}
    fn toggle_directory(&mut self,p:PathBuf){
        if p==self.cwd.parent().unwrap_or(Path::new("")){self.enter_directory(&p);return;}
        if p.is_dir(){if !self.expanded_dirs.insert(p.clone()){self.expanded_dirs.remove(&p);}self.refresh_tree();}
        else{self.open_path(p);}
    }

    fn enter_directory(&mut self,p:&Path){
        if p.is_dir(){self.cwd=p.to_path_buf();self.expanded_dirs.clear();self.tree_offset=0;self.selected=None;self.refresh_tree();self.refresh_git();self.message=format!("Folder: {}",p.display());}
        else {self.open_path(p.to_path_buf());}
    }
    fn new_untitled(&mut self){
        self.untitled_seq+=1;
        let path=self.cwd.join(format!("untitled-{}.txt", self.untitled_seq));
        match Document::open(path){Ok(d)=>{self.docs.push(d);self.active=self.docs.len()-1;self.welcome=false;},Err(e)=>self.message=format!("Cannot create tab: {e}")}
    }
    fn save_all(&mut self){
        let mut count=0; let mut errors=Vec::new();
        for d in &mut self.docs {if d.dirty {match d.save(){Ok(())=>{count+=1;let _=fs::remove_file(d.recovery_path());},Err(e)=>errors.push(e.to_string())}}}
        self.refresh_tree();self.refresh_git();self.message=if errors.is_empty(){format!("Saved {count} file(s)")}else{format!("Save errors: {}",errors.join(", "))};
    }
    fn open_recent(&mut self){
        if let Some(p)=self.recent.last().cloned(){self.open_path(p);}else{self.message="No recently opened files yet".into();}
    }
    fn open_path(&mut self,p:PathBuf){
        let path=if p.is_absolute(){p}else{self.cwd.join(p)};
        if path.is_dir(){self.enter_directory(&path);return;}
        if let Some(i)=self.docs.iter().position(|d|d.path==path){self.active=i;}
        else {match Document::open(path.clone()){Ok(d)=>{self.docs.push(d);self.active=self.docs.len()-1;},Err(e)=>self.message=format!("Open failed: {e}")}}
        if let Some(parent)=path.parent() {
            if parent.is_dir() && parent!=self.cwd.as_path() {
                self.cwd=parent.to_path_buf();
                self.expanded_dirs.clear();self.tree_offset=0;
                self.selected=Some(path.clone());
                self.refresh_tree();
                self.refresh_git();
            }
        }
        if !self.recent.contains(&path){self.recent.push(path);}
        self.completion.clear();self.welcome=false;self.needs_draw=true;
    }
    fn save(&mut self){
        let recovery=self.doc().recovery_path();
        match self.doc_mut().save(){Ok(())=>{let _=fs::remove_file(recovery);self.message="File saved".into();self.refresh_tree();self.refresh_git();},Err(e)=>self.message=format!("Save failed: {e}")}
    }
    fn save_as(&mut self,name:String){
        if let Some(path)=self.safe_path(&name){
            if path.exists(){self.message="Destination exists; choose another name".into();return;}
            self.doc_mut().path=path;
            self.save();
        }
    }
    fn safe_path(&mut self,s:&str)->Option<PathBuf>{
        let s=s.trim();
        if s.is_empty(){self.message="Name cannot be empty".into();return None;}
        let p=Path::new(s);
        // Restrict file management operations to a single item in the current folder.
        if p.components().count()!=1 || p.file_name().and_then(|v|v.to_str())!=Some(s) || s=="." || s==".." {
            self.message="Enter a name only (no slashes or parent paths)".into();return None;
        }
        Some(self.cwd.join(p))
    }
    fn create(&mut self,name:String,folder:bool){
        if let Some(_validated)=self.safe_path(&name){
            let parent=self.selected.as_ref().and_then(|s|if s.is_dir(){Some(s.clone())}else{s.parent().map(Path::to_path_buf)})
                .filter(|p|p.starts_with(&self.cwd)).unwrap_or_else(||self.cwd.clone());
            let p=parent.join(&name);
            if p.exists(){self.message="File or folder already exists".into();return;}
            let r=if folder{fs::create_dir(&p)}else{fs::OpenOptions::new().create_new(true).write(true).open(&p).map(|_|())};
            match r{Ok(())=>{self.refresh_tree();self.selected=Some(p.clone());if !folder{self.open_path(p);}self.message="Created successfully".into();},Err(e)=>self.message=format!("Create failed: {e}")}
        }
    }
    fn rename_selected(&mut self,name:String){
        let Some(old)=self.selected.clone() else {self.message="Select a file in the tree first".into();return;};
        if let Some(_validated)=self.safe_path(&name){
            let dest=old.parent().unwrap_or(&self.cwd).join(&name);
            if dest.exists(){self.message="Destination already exists".into();return;}
            match fs::rename(&old,&dest){Ok(())=>{for d in &mut self.docs{if d.path==old{d.path=dest.clone();}}self.selected=Some(dest);self.refresh_tree();self.message="Renamed".into();},Err(e)=>self.message=format!("Rename failed: {e}")}
        }
    }
    fn delete_selected(&mut self,s:String){
        if s!="DELETE"{self.message="Deletion cancelled".into();return;}
        let Some(path)=self.selected.clone() else {return;};
        if self.docs.iter().any(|d|d.path==path && d.dirty){self.message="Save or close unsaved file first".into();return;}
        // Never recursive: only delete empty directories.
        let result=if path.is_dir(){fs::remove_dir(&path)}else{fs::remove_file(&path)};
        match result{Ok(())=>{self.docs.retain(|d|d.path!=path);if self.docs.is_empty(){if let Ok(d)=Document::open(self.cwd.join("untitled.txt")){self.docs.push(d);}}self.active=self.active.min(self.docs.len()-1);self.selected=None;self.refresh_tree();self.message="Deleted".into();},Err(e)=>self.message=format!("Delete failed (folders must be empty): {e}")}
    }
    fn find(&mut self,s:&str){
        if s.is_empty(){return;}
        let d=self.doc();let mut result=None;
        for offset in 0..d.lines.len(){
            let row=(d.y+offset)%d.lines.len();let line:String=d.lines[row].iter().collect();
            let start=if offset==0{d.x.saturating_add(1)}else{0};
            let bytes=line.char_indices().nth(start).map(|(i,_)|i).unwrap_or(line.len());
            if let Some(i)=line[bytes..].find(s){let x=start+line[bytes..bytes+i].chars().count();result=Some((row,x));break;}
        }
        if let Some((y,x))=result{let d=self.doc_mut();d.y=y;d.x=x;self.message=format!("Found: {s}");}else{self.message=format!("Not found: {s}");}
    }
    fn submit_prompt(&mut self){
        let old=std::mem::replace(&mut self.prompt,Prompt::None);
        match old {Prompt::None=>{},Prompt::Find(s)=>self.find(&s),Prompt::Open(s)=>{if !s.trim().is_empty(){self.open_path(PathBuf::from(s));}},
            Prompt::NewFile(s)=>self.create(s,false),Prompt::NewFolder(s)=>self.create(s,true),Prompt::Rename(s)=>self.rename_selected(s),
            Prompt::SaveAs(s)=>self.save_as(s),Prompt::DeleteConfirm(s)=>self.delete_selected(s),Prompt::ReplaceFrom(s)=>self.prompt=Prompt::ReplaceTo(s,String::new()),Prompt::ReplaceTo(from,to)=>{let count=self.doc_mut().replace_all(&from,&to);self.message=format!("Replaced {count} occurrence(s)");},
            Prompt::RunPlugin(id)=>self.run_plugin(id),Prompt::PluginConsent(answer)=>{if let Some(p)=self.plugin_pending.take(){if answer=="YES"{match plugins::trust(&p){Ok(())=>self.execute_plugin(p),Err(e)=>self.message=format!("Trust error: {e}")}}else{self.message="Plugin execution cancelled".into();}}}}
    }
    fn open_menu(&mut self,m:Menu){self.context=None;self.menu=if self.menu==Some(m){None}else{Some(m)};self.needs_draw=true;}
    fn do_action(&mut self,action:&str){
        self.menu=None;self.context=None;self.needs_draw=true;
        match action {
            "New Text File"=>self.new_untitled(),
            "New File"|"New File in Folder..."=>self.prompt=Prompt::NewFile(String::new()),
            "New Folder"|"New Folder..."=>self.prompt=Prompt::NewFolder(String::new()),
            "Open File..."=>self.picker_requested=Some(Picker::File),
            "Open Folder..."=>self.picker_requested=Some(Picker::Folder),
            "Open Recent"=>self.open_recent(),
            "Save"=>self.save(),
            "Save As..."=>self.picker_requested=Some(Picker::SaveAs),
            "Save All"=>self.save_all(),
            "Auto Save"=>{self.auto_save=!self.auto_save;self.message=format!("Auto Save: {}",if self.auto_save{"ON"}else{"OFF"});},
            "Close Folder"=>{if self.docs.iter().any(|d|d.dirty){self.message="Save modified tabs before closing folder".into();}else{self.cwd=env::current_dir().unwrap_or_else(|_|self.cwd.clone());self.config.show_tree=false;self.refresh_tree();}},
            "Rename selected"=>{if let Some(p)=&self.selected{self.prompt=Prompt::Rename(p.file_name().unwrap_or_default().to_string_lossy().to_string());}else{self.message="Select a file in the tree first".into();}},
            "Delete selected"=>{if self.selected.is_some(){self.prompt=Prompt::DeleteConfirm(String::new());}else{self.message="Select a file in the tree first".into();}},
            "Close Editor"|"Close Tab"=>self.close_tab(),"Exit"|"Quit"=>{self.quit_armed=true;},
            "Marketplace..."|"Extensions Marketplace..."=>self.market.open(),
            "Installed Plugins"=>self.market.installed(),
            "Refresh Marketplace"=>{self.market.open();self.market.refresh();},
            "Toggle File Tree"=>self.config.show_tree=!self.config.show_tree,
            "Git Changes"=>{self.sidebar_git=!self.sidebar_git;self.tree_offset=0;self.config.show_tree=true;self.refresh_git();},
            "Refresh Git"=>self.refresh_git(),"Refresh Files"=>self.refresh_tree(),
            "Next Tab"=>self.active=(self.active+1)%self.docs.len(),
            "Previous Tab"=>self.active=(self.active+self.docs.len()-1)%self.docs.len(),
            "Toggle Vim Mode"=>self.vim_normal=!self.vim_normal,
            "Open Shell"=>self.shell_requested=true,"Terminal Panel"=>{self.terminal.toggle();self.problems_visible=false;},"Problems"=>{self.problems_visible=!self.problems_visible;self.terminal.visible=false;self.terminal.focus=false;},"Run Plugin..."=>self.prompt=Prompt::RunPlugin(String::new()),"Settings..."=>self.settings_requested=true,"Undo"=>self.doc_mut().undo(),"Redo"=>self.doc_mut().redo(),
            "Cut"|"Copy"|"Paste"|"Select All"|"Select Line"|"Duplicate Line"|"Delete Line"|"Comment/Uncomment"=>self.context_action(ContextTarget::Editor,action),
            "Find..."=>self.prompt=Prompt::Find(String::new()),"Replace..."=>self.prompt=Prompt::ReplaceFrom(String::new()),"Delete Character"=>self.doc_mut().delete(),
            "Keyboard shortcuts"=>self.message="F9 Extensions; Ctrl+O Folder; F4 File; Ctrl+Space LSP; Ctrl+G Git; Ctrl+` Terminal; F8 Problems".into(),
            "About Snack"=>self.message=format!("Snack {} • Rust • Tokyo Night • GTK file picker", env!("CARGO_PKG_VERSION")),_=>{}
        }
    }
    fn close_tab(&mut self){if self.doc().dirty{self.message="Save this tab before closing".into();}else if self.docs.len()>1{self.docs.remove(self.active);self.active=self.active.min(self.docs.len()-1);}else{self.message="Cannot close last tab".into();}}
    fn dashboard_activate(&mut self,choice:usize) {
        match choice {
            0 => {self.dashboard=false;self.welcome=false;self.config.show_tree=false;self.message="New buffer | Ctrl+Shift+S Save As".into();},
            1 => {self.dashboard=false;self.picker_requested=Some(Picker::File);},
            2 => {self.dashboard=false;self.picker_requested=Some(Picker::Folder);},
            3 => {self.dashboard=false;self.market.open();},
            4 => {self.dashboard=false;self.settings_requested=true;},
            5 => {self.quit_armed=true;},
            _ => {}
        }
        self.needs_draw=true;
    }
    fn handle_key(&mut self,key:KeyEvent)->bool{
        if key.kind!=KeyEventKind::Press && key.kind!=KeyEventKind::Repeat{return false;}
        self.needs_draw=true;
        if self.dashboard {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.dashboard_selected=(self.dashboard_selected+5)%6,
                KeyCode::Down | KeyCode::Char('j') => self.dashboard_selected=(self.dashboard_selected+1)%6,
                KeyCode::Enter => self.dashboard_activate(self.dashboard_selected),
                KeyCode::Char('n') => self.dashboard_activate(0),
                KeyCode::Char('f') => self.dashboard_activate(1),
                KeyCode::Char('o') => self.dashboard_activate(2),
                KeyCode::Char('m') | KeyCode::F(9) => self.dashboard_activate(3),
                KeyCode::Char('s') => self.dashboard_activate(4),
                KeyCode::Char('q') | KeyCode::Esc => self.dashboard_activate(5),
                _=>{}
            }
            return false;
        }
        if key.code==KeyCode::F(9) {
            if self.market.visible {self.market.close();}else{
                self.menu=None;self.context=None;self.terminal.focus=false;
                self.completion.clear();self.market.open();
            }
            return false;
        }
        if self.market.visible {
            let action=self.market.key(key);
            if let marketplace_ui::UiAction::Run(id)=action {self.run_plugin(id);}
            return false;
        }
        let ctrl=key.modifiers.contains(KeyModifiers::CONTROL);
        if key.code==KeyCode::Esc && (self.context.is_some() || self.menu.is_some()) {
            self.context=None;self.menu=None;return false;
        }
        if self.terminal.visible && self.terminal.focus && !self.prompt.active(){
            if ctrl && matches!(key.code,KeyCode::Char('`')) {self.terminal.toggle();return false;}
            match key.code {
                KeyCode::Esc=>{self.terminal.focus=false;self.message="Terminal focus released".into();},
                KeyCode::Enter=>self.terminal.execute(self.cwd.clone()),
                KeyCode::Backspace=>{self.terminal.input.pop();},
                KeyCode::Char(c) if !ctrl=>self.terminal.input.push(c),
                KeyCode::Char('v') if ctrl=>{
                    let paste=if self.clipboard_local_only {self.clipboard.clone()} else {
                        clipboard::read().ok().filter(|s|!s.is_empty()).unwrap_or_else(||self.clipboard.clone())
                    };
                    self.terminal.input.push_str(&paste.replace('\n'," ").replace('\r'," "));
                },
                _=>{}
            }return false;
        }
        if self.prompt.active(){self.completion.clear();}
        if !self.completion.is_empty() && !ctrl {
            match key.code {
                KeyCode::Esc=>{self.completion.clear();return false;}
                KeyCode::Up=>{self.completion_selected=(self.completion_selected+self.completion.len()-1)%self.completion.len();return false;}
                KeyCode::Down=>{self.completion_selected=(self.completion_selected+1)%self.completion.len();return false;}
                KeyCode::Tab=>{self.apply_completion();return false;}
                _=>{}
            }
        }
        if self.prompt.active(){match key.code {KeyCode::Esc=>self.prompt=Prompt::None,KeyCode::Enter=>self.submit_prompt(),KeyCode::Backspace=>{if let Some(s)=self.prompt.text_mut(){s.pop();}},KeyCode::Char(c) if !ctrl=>{if let Some(s)=self.prompt.text_mut(){s.push(c);}},_=>{}}return false;}
        if self.menu.is_some() || self.context.is_some(){if key.code==KeyCode::Esc{self.menu=None;self.context=None;return false;}self.menu=None;self.context=None;}
        if ctrl && key.code==KeyCode::Char('`'){self.terminal.toggle();return false;}
        if ctrl && matches!(key.code, KeyCode::Char(' ')|KeyCode::Null) {
            self.refresh_completion(true);
            self.request_lsp();
            if self.completion.is_empty(){self.message="No completion suggestions".into();}
            return false;
        }
        if ctrl {
            self.completion.clear();
            // Ctrl+Shift+O can only be distinguished from Ctrl+O in terminals that
            // report modifier keys separately (e.g. Kitty keyboard protocol).
            // In other terminals, File > Open File... and F4 are reliable fallbacks.
            match key.code {
                KeyCode::Char(ch) => match ch.to_ascii_lowercase() {
                    'o' => self.picker_requested = Some(if key.modifiers.contains(KeyModifiers::SHIFT) {
                        Picker::File
                    } else {
                        Picker::Folder
                    }),
                    's' => self.save(),
                    'a' => self.doc_mut().select_all(),
                    'c' => self.copy_text(self.doc().copy_selection_or_line()),
                    'x' => self.cut_text(),
                    'v' => self.paste_text(),
                    'q' => {
                        if self.docs.iter().any(|d|d.dirty) {
                            if self.message == "Unsaved files: Ctrl+Q again to discard" { return true; }
                            self.message = "Unsaved files: Ctrl+Q again to discard".into();
                        } else { return true; }
                    },
                    'f' => self.prompt = Prompt::Find(String::new()),
                    'h' => self.prompt = Prompt::ReplaceFrom(String::new()),
                    'n' => self.new_untitled(),
                    'w' => self.close_tab(),
                    'z' => self.doc_mut().undo(),
                    'y' => self.doc_mut().redo(),
                    'b' => self.config.show_tree = !self.config.show_tree,
                    'g' => {self.sidebar_git=!self.sidebar_git;self.tree_offset=0;self.config.show_tree=true;self.refresh_git();},
                    't' => self.shell_requested = true,
                    'p' => self.prompt=Prompt::RunPlugin(String::new()),
                    'd' => {self.problems_visible=!self.problems_visible;self.terminal.visible=false;self.terminal.focus=false;},

                    _ => {}
                },
                _ => {}
            }
            return false;
        }
        if key.code == KeyCode::F(8) {self.problems_visible=!self.problems_visible;self.terminal.visible=false;self.terminal.focus=false;return false;}
        if key.code == KeyCode::F(4) {
            self.picker_requested = Some(Picker::File);
            return false;
        }
        if self.vim_normal{self.completion.clear();match key.code {KeyCode::Char('i')=>self.vim_normal=false,KeyCode::Char('h')=>self.doc_mut().movement(KeyCode::Left),KeyCode::Char('j')=>self.doc_mut().movement(KeyCode::Down),KeyCode::Char('k')=>self.doc_mut().movement(KeyCode::Up),KeyCode::Char('l')=>self.doc_mut().movement(KeyCode::Right),KeyCode::Char('u')=>self.doc_mut().undo(),KeyCode::Char('/')=>self.prompt=Prompt::Find(String::new()),other=>self.doc_mut().movement(other)}return false;}
        let changing=matches!(key.code,KeyCode::Char(_)|KeyCode::Backspace|KeyCode::Delete);
        match key.code {
            KeyCode::Esc=>{self.doc_mut().selection_anchor=None;self.vim_normal=true;self.message="Vim NORMAL (i = insert)".into();},
            KeyCode::Char(c)=>{self.welcome=false;
                if self.doc().selection_range().is_some(){self.doc_mut().replace_selection(&c.to_string());}
                else{self.doc_mut().insert_smart(c);}
            },
            KeyCode::Tab=>{
                let spaces=" ".repeat(self.config.tab_width);
                self.doc_mut().replace_selection(&spaces);
            },
            KeyCode::Enter=>{if self.doc().selection_range().is_some(){self.doc_mut().replace_selection("\n");}
                else{self.doc_mut().newline();}},
            KeyCode::Backspace=>{if !self.doc_mut().delete_selection(){self.doc_mut().backspace();}},
            KeyCode::Delete=>{if !self.doc_mut().delete_selection(){self.doc_mut().delete();}},
            other=>{
                let shift=key.modifiers.contains(KeyModifiers::SHIFT);
                if shift {if self.doc().selection_anchor.is_none(){let d=self.doc();let pos=(d.y,d.x);self.doc_mut().selection_anchor=Some(pos);}}
                else {self.doc_mut().selection_anchor=None;}
                self.doc_mut().movement(other);
            }
        }
        if changing{self.refresh_completion(false);}else{self.completion.clear();}
        false
    }
    fn sidebar_width(&self,w:u16)->u16{if self.config.show_tree && w>=64{24}else{0}}
    fn handle_mouse(&mut self,m:crossterm::event::MouseEvent,w:u16,h:u16){
        if self.dashboard {
            if matches!(m.kind,MouseEventKind::Down(MouseButton::Left)) {
                let top=(h/2).saturating_sub(2);
                if m.row>=top && m.row<top+12 && (m.row-top)%2==0 {
                    self.dashboard_selected=((m.row-top)/2) as usize;
                    self.dashboard_activate(self.dashboard_selected);
                }
            }
            return;
        }
        if !matches!(m.kind,MouseEventKind::ScrollDown | MouseEventKind::ScrollUp |
            MouseEventKind::Down(MouseButton::Right) | MouseEventKind::Down(MouseButton::Left) |
            MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)) {return;}
        if self.market.visible {
            let action=self.market.mouse(m,w,h);
            if let marketplace_ui::UiAction::Run(id)=action {self.run_plugin(id);}
            self.needs_draw=true;
            return;
        }
        let side=self.sidebar_width(w);
        if matches!(m.kind,MouseEventKind::Up(MouseButton::Left)){self.mouse_anchor=None;return;}
        self.needs_draw=true;
        if matches!(m.kind,MouseEventKind::ScrollDown|MouseEventKind::ScrollUp){
            if let Some(ctx)=self.context.as_mut(){
                if ctx.contains(m.column,m.row){
                    if m.kind==MouseEventKind::ScrollDown {ctx.offset=(ctx.offset+1).min(ctx.items.len().saturating_sub(ctx.height));}
                    else{ctx.offset=ctx.offset.saturating_sub(1);}return;
                }
            }
            let down=m.kind==MouseEventKind::ScrollDown;
            if side>0 && m.column<side {let count=if self.sidebar_git{self.git_changes.len()}else{self.tree.len()};
                self.tree_offset=if down{(self.tree_offset+3).min(count.saturating_sub(1))}else{self.tree_offset.saturating_sub(3)};
            }else if self.terminal.visible && m.row>=h.saturating_sub(10){
                // The command runner currently displays the last 6 lines and has no scrollback viewport.
            }else{let d=self.doc_mut();d.y=if down{(d.y+3).min(d.lines.len()-1)}else{d.y.saturating_sub(3)};d.x=d.x.min(d.lines[d.y].len());}
            return;
        }
        if matches!(m.kind,MouseEventKind::Drag(MouseButton::Left)) {
            if let Some(anchor)=self.mouse_anchor {
                let end=h.saturating_sub(2+if self.terminal.visible||self.problems_visible{8}else{0});
                if m.row>=3 && m.row<end && m.column>=side+5 {
                    let d=self.doc_mut();let row=(d.top+(m.row-3) as usize).min(d.lines.len()-1);
                    let x=(d.left+(m.column-side-5) as usize).min(d.lines[row].len());
                    d.selection_anchor=Some(anchor);d.y=row;d.x=x;
                    self.completion.clear();
                }
            }
            return;
        }
        if matches!(m.kind,MouseEventKind::Down(MouseButton::Right)) {
            if self.prompt.active(){return;}
            let target=self.context_target_at(m.column,m.row,w,h);
            match &target {
                ContextTarget::Explorer(path)|ContextTarget::Git(path)=>self.selected=path.clone(),
                ContextTarget::Editor=>{
                    // Right click a different position moves the caret unless a selection is active there.
                    if m.column>=side+5 && m.row>=3 {
                        let d=self.doc_mut();let row=(d.top+(m.row-3) as usize).min(d.lines.len()-1);
                        let col=(d.left+(m.column-side-5) as usize).min(d.lines[row].len());
                        if !d.is_selected(row,col){d.y=row;d.x=col;d.selection_anchor=None;}
                    }
                },
                _=>{},
            }
            self.context=Some(ContextMenu::new(target,m.column,m.row,w,h));
            self.menu=None;self.completion.clear();return;
        }
        if self.prompt.active(){return;}
        // A left-click on an open context menu always takes precedence over the editor below it.
        if let Some(ctx)=self.context.take(){
            if ctx.contains(m.column,m.row){
                let index=ctx.offset+(m.row-ctx.y) as usize;
                if let Some(action)=ctx.items.get(index).copied(){self.context_action(ctx.target,action);}
                return;
            }
        }
        if !self.completion.is_empty() {
            let d=self.doc();let cy=d.y.saturating_sub(d.top)+3;
            let cx=d.x.saturating_sub(d.left)+5+side as usize;
            let count=self.completion.len().min(5);
            let popup_y=if cy+count+1>=(h-2) as usize{cy.saturating_sub(count)}else{cy+1};
            let x=cx.min((w as usize).saturating_sub(42));
            if (m.row as usize)>=popup_y && (m.row as usize)<popup_y+count &&
                (m.column as usize)>=x && (m.column as usize)<x+42 {
                let base=if self.completion_selected>=5{self.completion_selected-4}else{0};
                self.completion_selected=(base+(m.row as usize-popup_y)).min(self.completion.len()-1);
                self.apply_completion();return;
            }
            self.completion.clear();
        }
        if let Some(menu)=self.menu.take(){
            let x=menu_x(menu);let items=menu.items();
            if m.column>=x && m.column<x+28 && m.row>=1 && (m.row-1)<items.len() as u16{
                self.do_action(items[(m.row-1) as usize]);return;
            }
        }
        if m.row==0 {
            for menu in [Menu::File,Menu::Edit,Menu::View,Menu::Help,Menu::Extensions]{
                let x=menu_x(menu);
                if m.column>=x && m.column<x+menu.title().len() as u16+2 {self.open_menu(menu);return;}
            }
        }
        if self.terminal.visible && m.row>=h.saturating_sub(10) && m.row<h.saturating_sub(2){
            self.terminal.focus=true;return;
        }
        if self.problems_visible && m.row>=h.saturating_sub(10) && m.row<h.saturating_sub(2){return;}
        if m.row==1 && m.column>=side {
            if let Some(idx)=self.tab_at(m.column,side){self.active=idx;self.doc_mut().selection_anchor=None;}
        }else if side>0 && m.column<side && m.row>=3 && m.row<h.saturating_sub(2+if self.terminal.visible||self.problems_visible{8}else{0}){
            let idx=self.tree_offset+(m.row-3) as usize;
            if self.sidebar_git {
                if let Some(c)=self.git_changes.get(idx).cloned(){self.selected=Some(c.path.clone());self.open_path(c.path);}
            }else if let Some(p)=self.tree.get(idx).cloned(){self.selected=Some(p.clone());self.toggle_directory(p);}
        }else if m.row>=3 && m.row<h.saturating_sub(2+if self.terminal.visible||self.problems_visible{8}else{0}) && m.column>=side+5{
            let d=self.doc_mut();let row=d.top+(m.row-3) as usize;
            if row<d.lines.len(){d.y=row;d.x=(d.left+(m.column-side-5) as usize).min(d.lines[row].len());d.selection_anchor=None;
                let pos=(d.y,d.x);self.mouse_anchor=Some(pos);}
        }
    }
    fn autosave(&mut self){if self.config.autosave_seconds==0{return;}for d in &mut self.docs{if d.dirty && d.last_autosave.elapsed().as_secs()>=self.config.autosave_seconds{if let Err(e)=d.autosave_recovery(){self.message=format!("Recovery error: {e}");self.needs_draw=true;}}}}
}
fn menu_x(m:Menu)->u16{match m{Menu::File=>9,Menu::Edit=>16,Menu::View=>23,Menu::Help=>30,Menu::Extensions=>37}}
#[derive(Clone,Copy,PartialEq,Eq)]
struct Cell { ch:char, fg:Color, bg:Color }
#[derive(Clone)]
struct Frame {width:u16,height:u16,cells:Vec<Cell>}
impl Frame {
    fn new(width:u16,height:u16)->Self{Self{width,height,cells:vec![Cell{ch:' ',fg:TEXT,bg:BG};width as usize*height as usize]}}
    fn put(&mut self,x:u16,y:u16,ch:char,fg:Color,bg:Color){
        if x<self.width && y<self.height {let i=y as usize*self.width as usize+x as usize;self.cells[i]=Cell{ch,fg,bg};}
    }
    fn write(&mut self,x:u16,y:u16,text:&str,fg:Color,bg:Color,max:usize){
        for (i,c) in text.chars().take(max).enumerate(){if x as usize+i>=self.width as usize{break;}self.put(x+i as u16,y,c,fg,bg);}
    }
    fn fill(&mut self,x:u16,y:u16,width:usize,bg:Color){for i in 0..width{if x as usize+i>=self.width as usize{break;}self.put(x+i as u16,y,' ',TEXT,bg);}}
    fn flush_diff(&self,old:Option<&Frame>,out:&mut Stdout)->io::Result<()> {
        // Update only changed cells, in consecutive same-style runs. Never erase or repaint a full screen.
        let full=old.map(|p|p.width!=self.width||p.height!=self.height).unwrap_or(true);
        for y in 0..self.height {
            let mut x=0;
            while x<self.width {
                let idx=y as usize*self.width as usize+x as usize;
                let cell=self.cells[idx];
                let changed=full || old.is_some_and(|p|p.cells[idx]!=cell);
                if !changed{x+=1;continue;}
                let begin=x;let mut text=String::new();
                while x<self.width {
                    let idx=y as usize*self.width as usize+x as usize;
                    let c=self.cells[idx];
                    let diff=full || old.is_some_and(|p|p.cells[idx]!=c);
                    if !diff || c.fg!=cell.fg || c.bg!=cell.bg {break;}
                    text.push(c.ch);x+=1;
                }
                queue!(out,MoveTo(begin,y),SetForegroundColor(cell.fg),SetBackgroundColor(cell.bg),Print(text))?;
            }
        }
        Ok(())
    }
}
fn color_for(line:&[char],idx:usize)->Color{
    let first_non_ws=line.iter().position(|c|!c.is_whitespace()).unwrap_or(line.len());
    let tail=&line[first_non_ws..];
    if tail.starts_with(&['/','/']) || tail.starts_with(&['#']){return MUTED;}
    if idx<line.len() && line[idx].is_ascii_digit(){return YELLOW;}
    if matches!(line.get(idx).copied(),Some('"'|'\'')){return GREEN;}
    let isword=|c:char|c.is_alphanumeric()||c=='_';if idx>=line.len()||!isword(line[idx]){return TEXT;}
    let mut a=idx;let mut b=idx;while a>0 && isword(line[a-1]){a-=1;}while b<line.len() && isword(line[b]){b+=1;}
    let word:String=line[a..b].iter().collect();
    if ["fn","let","mut","pub","use","mod","struct","impl","enum","match","if","else","for","while","loop","return","async","await","def","class","import","from","as","const","var","function","export","default","package","func","type","public","private","static","void","int","char","float","double","true","false","null","None","self","new","break","continue","interface","extends","try","catch","throw","echo","then","fi","do","done"].contains(&word.as_str()){PURPLE}
    else if ["String","Vec","Some","Ok","Err","Result","Option","println","print","main","bool","usize","i32","u32","i64","u64"].contains(&word.as_str()){BLUE}else{TEXT}
}
fn draw_dashboard(app:&mut App,out:&mut Stdout,w:u16,h:u16)->io::Result<()> {
    let mut frame=Frame::new(w,h);
    if w<42 || h<19 {
        frame.write(1,1,"Snack | enlarge terminal (42x19)",BLUE,BG,w.saturating_sub(2) as usize);
    } else {
        let pulse=["·","✦","✧","✦"];
        let logo=[
            "  _____ _   _    _    ____ _  __ ",
            " / ____| \\ | |  / \\  / ___| |/ / ",
            "| (___ |  \\| | / _ \\| |   | ' /  ",
            " \\___ \\| |\\  |/ ___ \\ |___| . \\  ",
            " |____/|_| \\_/_/   \\_\\____|_|\\_\\ ",
        ];
        let logo_w=37u16;
        let lx=w.saturating_sub(logo_w)/2;
        let y0=(h/2).saturating_sub(10);
        let reveal=(app.dashboard_start.elapsed().as_millis()/70) as usize;
        for (i,line) in logo.iter().enumerate() {
            let shown=reveal.saturating_sub(i*5).min(line.chars().count());
            let txt:String=line.chars().take(shown).collect();
            frame.write(lx,y0+i as u16,&txt,if i%2==0{BLUE}else{PURPLE},BG,w as usize);
        }
        let subtitle=format!("{}  Snack v{}  •  Plugin-first terminal editor  {}",pulse[app.dashboard_tick%4],env!("CARGO_PKG_VERSION"),pulse[app.dashboard_tick%4]);
        frame.write(w.saturating_sub(subtitle.chars().count() as u16)/2,y0+6,&subtitle,MUTED,BG,w as usize);
        let labels=["New File","Open File","Open Folder","Extensions Marketplace","Settings","Quit"];
        let keys=["N","F","O","M","S","Q"];
        let top=h/2-2;
        let x=w.saturating_sub(38)/2;
        for i in 0..6 {
            let y=top+(i*2) as u16;
            let selected=app.dashboard_selected==i;
            let bg=if selected{PANEL}else{BG};
            frame.fill(x,y,38,bg);
            let label=format!("  {}  {:<27} {}",if selected{"›"}else{" "},labels[i],keys[i]);
            frame.write(x,y,&label,if selected{GREEN}else{TEXT},bg,38);
        }
        let help="↑↓ / j k navigate    Enter select    Esc quit";
        frame.write(w.saturating_sub(help.len() as u16)/2,h.saturating_sub(3),help,MUTED,BG,w as usize);
        frame.write(2,h.saturating_sub(1),"SNACK  •  Welcome",BLUE,BG,w.saturating_sub(4) as usize);
    }
    frame.flush_diff(app.prev.as_ref(),out)?;
    app.prev=Some(frame);
    Ok(())
}

fn draw(app:&mut App,out:&mut Stdout,w:u16,h:u16)->io::Result<()> {
    if app.dashboard {return draw_dashboard(app,out,w,h);}

    let mut frame=Frame::new(w,h);
    if w<40||h<12 {frame.write(0,0,"Enlarge terminal for Snack",TEXT,BG,w as usize);}
    else {
        let side=app.sidebar_width(w);let cw=(w-side) as usize;let panel_rows=if app.terminal.visible || app.problems_visible{8}else{0};
        let rows=h.saturating_sub(5+panel_rows) as usize;
        let d=app.doc_mut();if d.y<d.top{d.top=d.y;}if rows>0 && d.y>=d.top+rows{d.top=d.y+1-rows;}
        let available=cw.saturating_sub(5).max(1);if d.x<d.left{d.left=d.x;}if d.x>=d.left+available{d.left=d.x+1-available;}
        frame.fill(0,0,w as usize,PANEL);
        frame.write(0,0," Snack   File   Edit   View   Help   Extensions                 0.6.4",TEXT,PANEL,w as usize);
        if side>0 {
            frame.fill(0,1,side as usize,PANEL);
            frame.write(0,1,if app.sidebar_git{" SOURCE CONTROL"}else{" EXPLORER"},BLUE,PANEL,side as usize);
            frame.write(0,2,if app.sidebar_git{" Ctrl+G: Explorer"}else{" Click folders to expand"},MUTED,BG,side as usize);
            if app.sidebar_git {
                for (i,change) in app.git_changes.iter().skip(app.tree_offset).take(rows).enumerate(){
                    let selected=app.selected.as_ref()==Some(&change.path);
                    let label=format!("{} {}",change.state,change.path.strip_prefix(&app.cwd).unwrap_or(&change.path).display());
                    if selected {frame.fill(0,(i+3) as u16,side as usize,PANEL);}
                    frame.write(0,(i+3) as u16,&label,if selected{YELLOW}else{GREEN},if selected{PANEL}else{BG},side as usize);
                }
            } else {
                for (i,p) in app.tree.iter().skip(app.tree_offset).take(rows).enumerate(){
                    let selected=app.selected.as_ref()==Some(p);
                    let parent=p==app.cwd.parent().unwrap_or(Path::new(""));
                    let depth=*app.tree_depth.get(p).unwrap_or(&0);
                    let mark=if parent{".."}else if p.is_dir() {if app.expanded_dirs.contains(p){"v "}else{"> "}}else{"  "};
                    let label=if parent{".. (parent)".to_string()}else{format!("{}{}{}", " ".repeat(depth.saturating_mul(2)),mark,p.file_name().unwrap_or_default().to_string_lossy())};
                    if selected{frame.fill(0,(i+3) as u16,side as usize,PANEL);}
                    frame.write(0,(i+3) as u16,&label,if selected{YELLOW}else if p.is_dir(){BLUE}else{TEXT},if selected{PANEL}else{BG},side as usize);
                }
            }
        }
        frame.fill(side,1,cw,PANEL);
        let mut tabs=String::new();for (i,d) in app.docs.iter().enumerate(){tabs.push_str(&format!(" {}{}{} ",if i==app.active{"["}else{" "},d.title(),if d.dirty{"*]"}else{"]"}));}
        frame.write(side,1,&tabs,BLUE,PANEL,cw);
        for row in 0..rows {
            let line_no=app.doc().top+row;
            if line_no>=app.doc().lines.len(){frame.write(side,(row+3) as u16,"~",MUTED,BG,cw);continue;}
            frame.write(side,(row+3) as u16,&format!("{:>4} ",line_no+1),MUTED,BG,5);
            let line=&app.doc().lines[line_no];let left=app.doc().left;
            for col in 0..available{let idx=left+col;if idx>=line.len(){break;}
                let c=line[idx];
                // Tabs / full-width glyphs are intentionally simplified for this alpha version.
                let c=if c=='\t'{' '}else if c.is_control(){' '}else{c};
                let selected=app.doc().is_selected(line_no,idx);
                let bg=if selected{Color::Rgb{r:65,g:81,b:115}}else{BG};
                frame.put(side+5+col as u16,(row+3) as u16,c,if selected{TEXT}else{color_for(line,idx)},bg);
            }
        }
        if !app.completion.is_empty() && !app.prompt.active() && !app.vim_normal {
            let cy=app.doc().y.saturating_sub(app.doc().top)+3;
            let cx=app.doc().x.saturating_sub(app.doc().left)+5+side as usize;
            if cy<(h-2) as usize {
                let count=app.completion.len().min(5);
                let above=(cy+count+1)>=(h-2) as usize;
                let popup_y=if above {cy.saturating_sub(count)} else {cy+1};
                let maxw=w as usize;
                let x=cx.min(maxw.saturating_sub(42));
                let width=42.min(maxw.saturating_sub(x));
                for i in 0..count {
                    let index=if app.completion_selected>=5 {app.completion_selected-4+i}else{i};
                    if let Some(c)=app.completion.get(index) {
                        let y=popup_y+i;
                        if y>1 && y<(h-2) as usize {
                            let selected=index==app.completion_selected;
                            let bg=if selected{Color::Rgb{r:59,g:66,b:93}}else{PANEL};
                            let txt=format!("{} {:<20} {}",if selected{">"}else{" "},c.label,c.description);
                            frame.fill(x as u16,y as u16,width,bg);
                            frame.write(x as u16,y as u16,&txt,if selected{YELLOW}else{TEXT},bg,width);
                        }
                    }
                }
            }
        }
        if app.welcome{
            let lines=["", "       +--------------------------+", "       |          SNACK           |", "       |     Rust code editor     |", "       +--------------------------+", "       File > Open Folder (GTK)", "       Ctrl+O: folder; Ctrl+Shift+O: file"];
            for (i,s) in lines.iter().enumerate(){let y=i+5;if y<(h-2) as usize{frame.write(side,y as u16,s,BLUE,BG,cw);}}
        }
        if app.problems_visible{
            let top=h.saturating_sub(10);
            frame.fill(0,top,w as usize,PANEL);
            frame.write(0,top,&format!(" PROBLEMS — {} diagnostic(s) | F8 to close",app.diagnostics.len()),YELLOW,PANEL,w as usize);
            for (i,line) in app.diagnostics.iter().take(7).enumerate(){
                let y=top+1+i as u16;if y<h-2{frame.write(1,y,line,RED,BG,w.saturating_sub(2) as usize);}
            }
            if app.diagnostics.is_empty(){frame.write(1,top+1,"No LSP diagnostics received; use Ctrl+Space to connect language server",MUTED,BG,w.saturating_sub(2) as usize);}
        }
        if app.terminal.visible{
            let top=h.saturating_sub(10);
            frame.fill(0,top,w as usize,PANEL);
            frame.write(0,top," TERMINAL — command runner (not PTY) | Esc = editor",BLUE,PANEL,w as usize);
            let count=app.terminal.lines.len();
            for (i,line) in app.terminal.lines.iter().skip(count.saturating_sub(6)).take(6).enumerate(){
                let y=top+1+i as u16;if y<h-2{frame.write(1,y,line,TEXT,BG,w.saturating_sub(2) as usize);}
            }
            frame.fill(0,h-3,w as usize,PANEL);
            let command=format!(" > {}{}",app.terminal.input,if app.terminal.running{"  [running]"}else{""});
            frame.write(0,h-3,&command,GREEN,PANEL,w as usize);
        }
        let mode=if app.vim_normal{"NORMAL"}else{"INSERT"};let git=if app.git.is_empty(){String::new()}else{format!(" | git:{}",app.git)};
        let status=format!(" {mode} | {}  Ln {}, Col {} | {}{}",app.doc().title(),app.doc().y+1,app.doc().x+1,app.message,git);
        frame.fill(0,h-2,w as usize,PANEL);
        frame.write(0,h-2,&status,TEXT,PANEL,w as usize);
        let foot=if app.prompt.active(){let label=app.prompt.label();let value=app.prompt.text_mut().map(|s|s.as_str()).unwrap_or("");format!(" {label}: {value}")}
            else{" F9 Extensions  ^S Save  ^O Folder  F4 File  ^G Git  ^` Terminal  F8 Problems  ^Space LSP  Right-click Menu".into()};
        frame.write(0,h-1,&foot,GREEN,BG,w as usize);
        if let Some(menu)=app.menu{let items=menu.items();let x=menu_x(menu);let menuwidth=28.min(w.saturating_sub(x)) as usize;
            for (i,item) in items.iter().enumerate(){let y=(i+1) as u16;if y<h-2{frame.fill(x,y,menuwidth,PANEL);frame.write(x,y,item,TEXT,PANEL,menuwidth);}}
        }
        if let Some(ctx)=&app.context {
            for i in 0..ctx.height {
                let row=ctx.y+i as u16;
                if row>=h.saturating_sub(2){break;}
                let label=ctx.items[ctx.offset+i];
                let bg=if i%2==0{PANEL}else{Color::Rgb{r:43,g:48,b:69}};
                frame.fill(ctx.x,row,ctx.width as usize,bg);
                frame.write(ctx.x,row,&format!("  {}",label),TEXT,bg,ctx.width as usize);
            }
            if ctx.offset>0 && ctx.height>0 {frame.put(ctx.x+ctx.width-1,ctx.y,'^',YELLOW,PANEL);}
            if ctx.offset+ctx.height<ctx.items.len() && ctx.height>0 {
                frame.put(ctx.x+ctx.width-1,ctx.y+ctx.height as u16-1,'v',YELLOW,PANEL);
            }
        }
    }
    if app.market.visible {
        let visible_rows=h.saturating_sub(9) as usize;
        app.market.view_offset(visible_rows);
        draw_marketplace(&mut frame,&app.market,w,h);
    }
    // Synchronized terminal updates, where supported, prevent intermediate frame flashes.
    write!(out,"\x1b[?2026h")?;
    queue!(out,Hide)?;
    frame.flush_diff(app.prev.as_ref(),out)?;
    if app.market.visible { /* Marketplace is a fully keyboard-driven screen; keep cursor hidden. */ }
    else if app.terminal.visible && app.terminal.focus && h>=4 && w>0 && !app.prompt.active(){
        let x=(3+app.terminal.input.chars().count()).min((w-1) as usize) as u16;
        queue!(out,MoveTo(x,h-3),Show)?;
    }else if h>0 && w>0 && app.prompt.active(){
        let label=app.prompt.label();let len=app.prompt.text_mut().map(|s|s.chars().count()).unwrap_or(0);
        let x=(label.chars().count()+3+len).min((w-1) as usize) as u16;
        queue!(out,MoveTo(x,h-1),Show)?;
    }else if app.menu.is_none() && app.context.is_none() && w>=40 && h>=12 {
        let d=app.doc();let side=app.sidebar_width(w);let cy=d.y.saturating_sub(d.top)+3;let cx=d.x.saturating_sub(d.left)+5+side as usize;
        if cy<(h-2-if app.terminal.visible || app.problems_visible{8}else{0}) as usize && cx<w as usize{queue!(out,MoveTo(cx as u16,cy as u16),Show)?;}
    }
    queue!(out,ResetColor)?;
    write!(out,"\x1b[?2026l")?;
    out.flush()?;
    app.prev=Some(frame);
    Ok(())
}
fn draw_marketplace(frame:&mut Frame,market:&marketplace_ui::Browser,w:u16,h:u16){
    // Fullscreen frame built in memory; the same diff renderer as the editor avoids flicker.
    if w<45 || h<12 {frame.fill(0,0,w as usize,BG);frame.write(1,1,"Enlarge terminal for Extensions Marketplace",YELLOW,BG,w as usize);return;}
    for y in 0..h {frame.fill(0,y,w as usize,BG);}
    frame.fill(0,0,w as usize,PANEL);
    frame.write(1,0,"SNACK | EXTENSIONS MARKETPLACE",BLUE,PANEL,(w-2) as usize);
    frame.write(w.saturating_sub(14),0,"F9 / Esc Close",MUTED,PANEL,14);
    frame.fill(0,1,w as usize,PANEL);
    let all=market.category==marketplace_ui::Category::Discover;
    frame.write(2,1,"[ Discover ]",if all{YELLOW}else{MUTED},PANEL,19);
    frame.write(22,1,"[ Installed ]",if !all{YELLOW}else{MUTED},PANEL,19);
    frame.write(2,2,&format!("Search: {}_",market.query),GREEN,BG,w.saturating_sub(3) as usize);
    frame.write(2,3,"Official: criteric/snack-plugins | GitHub Releases | SHA-256 verified",MUTED,BG,w.saturating_sub(3) as usize);
    let split=w/2;
    frame.fill(0,4,w as usize,PANEL);
    frame.write(2,4,"PLUGINS",BLUE,PANEL,split.saturating_sub(2) as usize);
    frame.write(split+1,4,"DETAILS",BLUE,PANEL,w.saturating_sub(split+1) as usize);
    for y in 5..h.saturating_sub(4){frame.put(split,y,'|',MUTED,BG);}
    let items=market.items();
    let visible=h.saturating_sub(9) as usize;
    for (row,item) in items.iter().skip(market.offset).take(visible).enumerate(){
        let y=row as u16+5;let picked=market.selected==market.offset+row;
        let bg=if picked{PANEL}else{BG};
        frame.fill(0,y,split as usize,bg);
        let mark=if item.installed.is_some(){"[+]"}else{"[ ]"};
        let line=format!("{} {} {}",if picked{">"}else{" "},mark,item.name);
        frame.write(1,y,&line,if picked{YELLOW}else{TEXT},bg,split.saturating_sub(2) as usize);
    }
    if items.is_empty(){frame.write(2,6,if market.is_loading(){"Loading..."}else{"No matching plugins"},MUTED,BG,split.saturating_sub(3) as usize);}
    if let Some(entry)=market.highlighted(){
        let x=split+2;let avail=w.saturating_sub(x+2) as usize;
        let mut fields=vec![
            ("Plugin",entry.name),
            ("ID",entry.id),
            ("Version",entry.version),
            ("Runtime",entry.runtime),
            ("Installed",entry.installed.unwrap_or_else(||"No".into())),
            ("About",entry.description),
        ];
        for (idx,(label,value)) in fields.drain(..).enumerate(){
            let row=6+idx as u16*2;
            if row>=h.saturating_sub(5){break;}
            frame.write(x,row,&format!("{label}:"),BLUE,BG,avail);
            if row+1<h.saturating_sub(4){frame.write(x,row+1,&value,TEXT,BG,avail);}
        }
    }
    frame.fill(0,h-3,w as usize,PANEL);
    frame.write(1,h-3,"[I] Install  [U] Update  [X] Remove  [P] Run  [F5] Refresh",GREEN,PANEL,w.saturating_sub(2) as usize);
    frame.fill(0,h-2,w as usize,PANEL);
    frame.write(1,h-2,&market.note,if market.busy{YELLOW}else{TEXT},PANEL,w.saturating_sub(2) as usize);
    frame.write(1,h-1,"Type to search | Backspace delete | / clear | Tab switch | Up/Down select | Enter details",MUTED,BG,w.saturating_sub(2) as usize);
    if let Some(change)=&market.confirm {
        let bw=60u16.min(w.saturating_sub(4));let left=w.saturating_sub(bw)/2;
        let y=h/2;
        for row in y.saturating_sub(2)..=(y+2).min(h.saturating_sub(1)){frame.fill(left,row,bw as usize,PANEL);}
        let msg=match change{marketplace_ui::Change::Install(_)=>"Install plugin?",marketplace_ui::Change::Remove(_)=>"Remove installed plugin?",marketplace_ui::Change::Update(_)=>"Update plugin?"};
        frame.write(left+2,y.saturating_sub(1),msg,YELLOW,PANEL,bw.saturating_sub(4) as usize);
        frame.write(left+2,y,change.id(),TEXT,PANEL,bw.saturating_sub(4) as usize);
        frame.write(left+2,y+1,"Y confirm / N or Esc cancel",GREEN,PANEL,bw.saturating_sub(4) as usize);
    }
}

fn enable_enhanced_keys(out: &mut Stdout) -> io::Result<()> {
    // Terminals without Kitty keyboard support ignore these sequences.
    execute!(out, PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES))
}
fn disable_enhanced_keys(out: &mut Stdout) -> io::Result<()> {
    execute!(out, PopKeyboardEnhancementFlags)
}
fn enter_screen(out:&mut Stdout)->io::Result<()>{
    terminal::enable_raw_mode()?;
    if let Err(error) = execute!(out,EnterAlternateScreen,EnableMouseCapture,Hide) {
        let _=terminal::disable_raw_mode();
        return Err(error);
    }
    // Do not abort the whole editor if the terminal cannot enable this mode.
    let _=enable_enhanced_keys(out);
    Ok(())
}
fn leave_screen(out:&mut Stdout)->io::Result<()>{
    let _=disable_enhanced_keys(out);
    let result=execute!(out,Show,DisableMouseCapture,LeaveAlternateScreen,ResetColor);
    let raw_result=terminal::disable_raw_mode();
    result?;
    raw_result
}
fn shell(out:&mut Stdout)->io::Result<()>{leave_screen(out)?;println!("Snack shell: type exit to return.");let sh=if cfg!(windows){env::var("COMSPEC").unwrap_or_else(|_|"cmd.exe".into())}else{env::var("SHELL").unwrap_or_else(|_|"/bin/sh".into())};let result=Command::new(sh).status();if let Err(e)=result{eprintln!("Shell failed: {e}");}enter_screen(out)}
fn suspend_dialog(out:&mut Stdout)->io::Result<()>{let _=disable_enhanced_keys(out);execute!(out,DisableMouseCapture,Show)?;terminal::disable_raw_mode()}
fn resume_dialog(out:&mut Stdout)->io::Result<()>{terminal::enable_raw_mode()?;execute!(out,EnableMouseCapture,Hide)?;let _=enable_enhanced_keys(out);Ok(())}

// Temporarily release the terminal while the native GTK file picker is open.
// Always restore it, even when zenity is unavailable or the user cancels.
fn run_picker(app: &mut App, out: &mut Stdout, picker: Picker) -> io::Result<()> {
    suspend_dialog(out)?;
    let selection=platform::pick(picker,&app.cwd,&app.doc().path);
    resume_dialog(out)?;
    app.prev=None;
    app.needs_draw=true;
    match selection {
        Ok(Some(path))=>match picker {
            Picker::File=>app.open_path(path),
            Picker::Folder=>{
                if path.is_dir(){app.enter_directory(&path);}
                else{app.message="Selected folder does not exist".into();}
            },
            Picker::SaveAs=>{
                app.doc_mut().path=path.clone();
                app.save();
                if let Some(parent)=path.parent(){
                    if parent.is_dir() && parent!=app.cwd.as_path(){
                        app.cwd=parent.to_path_buf();app.refresh_tree();app.update_git();
                    }
                }
            }
        },
        Ok(None)=>{},
        Err(e)=>app.message=format!("File picker error: {e}. Linux: install zenity; Windows: PowerShell/WinForms"),
    }
    Ok(())
}

fn run(app:&mut App,out:&mut Stdout)->io::Result<()>{
    let mut size=terminal::size()?;let mut last_autosave=Instant::now();
    loop{
        let current=terminal::size()?;if current!=size{size=current;app.prev=None;app.needs_draw=true;}
        if app.dashboard {
            let tick=(app.dashboard_start.elapsed().as_millis()/180) as usize;
            if tick!=app.dashboard_tick {app.dashboard_tick=tick;app.needs_draw=true;}
        }
        if app.market.poll(){app.plugins=plugins::load();app.needs_draw=true;}
        app.poll_lsp();
        if app.terminal.poll(){app.needs_draw=true;}
        if app.needs_draw{draw(app,out,size.0,size.1)?;app.needs_draw=false;}
        if app.quit_armed{if app.docs.iter().any(|d|d.dirty){app.quit_armed=false;app.message="Unsaved files: Ctrl+Q twice to discard".into();app.needs_draw=true;}else{break;}}
        if event::poll(Duration::from_millis(120))?{match event::read()?{Event::Key(key)=>{if app.handle_key(key){break;}},Event::Mouse(mouse)=>app.handle_mouse(mouse,size.0,size.1),Event::Resize(..)=>app.needs_draw=true,_=>{}}}
        if last_autosave.elapsed()>=Duration::from_secs(1){
            app.autosave();
            if app.auto_save && app.docs.iter().any(|d|d.dirty){app.save_all();app.needs_draw=true;}
            last_autosave=Instant::now();
        }
        if let Some(picker)=app.picker_requested.take(){run_picker(app,out,picker)?;app.needs_draw=true;app.prev=None;}
        if app.shell_requested{app.shell_requested=false;shell(out)?;app.needs_draw=true;app.prev=None;}
        if app.settings_requested {
            app.settings_requested=false;
            suspend_dialog(out)?;
            let result=settings::open(None);
            resume_dialog(out)?;
            if let Err(e)=result {app.message=format!("Settings failed: {e}");}
            else {app.config=Config::load();app.message="Settings updated".into();}
            app.needs_draw=true;
            app.prev=None;
        }
    }
    Ok(())
}
fn main()->io::Result<()>{
    let command:Vec<String>=env::args().skip(1).collect();
    match command.first().map(String::as_str) {
        Some("--version" | "-version" | "-V" | "version") => {
            println!("snack {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        },
        Some("--help" | "-h" | "help") => {
            println!("Snack {}\nUsage: snack [files...]\n       snack settings [--gui|--tui]\n       snack --version\nKeyboard: Ctrl+O open folder, Ctrl+Shift+O open file, F4 open file (fallback).\nMouse: right-click Explorer/editor/tabs/Git/terminal/status for context-aware menus.", env!("CARGO_PKG_VERSION"));
            return Ok(());
        },
        Some("settings") => return settings::open(command.get(1).map(String::as_str)),
        _ => {}
    }
    let paths:Vec<PathBuf>=env::args_os().skip(1).map(PathBuf::from).collect();
    let mut app=App::new(paths)?;let mut out=io::stdout();enter_screen(&mut out)?;
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||run(&mut app,&mut out)));
    let cleanup=leave_screen(&mut out);
    match result{Ok(r)=>{cleanup?;r},Err(p)=>{let _=cleanup;std::panic::resume_unwind(p)}}
}

#[cfg(test)]
mod keyboard_tests {
    use super::*;
    #[test]
    fn cli_version_is_a_package_version() {
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
    }
    #[test]
    fn keycode_shift_modifier_can_be_distinguished_when_terminal_reports_it() {
        let plain = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL);
        let shifted = KeyEvent::new(KeyCode::Char('O'), KeyModifiers::CONTROL | KeyModifiers::SHIFT);
        assert!(!plain.modifiers.contains(KeyModifiers::SHIFT));
        assert!(shifted.modifiers.contains(KeyModifiers::SHIFT));
    }
}

#[cfg(test)]
mod context_menu_tests {
    use super::*;
    #[test] fn editor_menu_contains_real_edit_actions(){
        let actions=context_items(&ContextTarget::Editor);
        for expected in ["Copy","Paste","Cut","Select All","Select Line","Duplicate Line","Delete Line","Comment/Uncomment","Undo","Redo","Find...","Replace..."] {
            assert!(actions.contains(&expected),"missing {expected}");
        }
    }
    #[test] fn menus_depend_on_target(){
        assert!(!context_items(&ContextTarget::Editor).contains(&"Rename selected"));
        assert!(context_items(&ContextTarget::Tab(0)).contains(&"Close Other Tabs"));
        assert!(context_items(&ContextTarget::Terminal).contains(&"Copy Terminal Output"));
        assert!(context_items(&ContextTarget::Git(None)).contains(&"Refresh Git"));
    }
    #[test] fn near_edge_menus_stay_in_view_and_scroll(){
        let ctx=ContextMenu::new(ContextTarget::Editor,77,26,80,28);
        assert!(ctx.x+ctx.width<=80);
        assert!(ctx.y+ctx.height as u16<=26);
        assert!(ctx.height<ctx.items.len());
    }
}
