//! Interactive Snack Marketplace, backed by the official registry.
//! All network operations run on worker threads so the editor redraw loop stays responsive.
//! Installing packages never executes plugin code; the editor asks separately before first run.
use std::{fs, sync::mpsc::{self, Receiver, TryRecvError}, thread};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use serde_json::Value;
use crate::{marketplace, plugins};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category { Discover, Installed }

#[derive(Clone, Debug)]
pub struct Item {
    pub id: String, pub name: String, pub version: String,
    pub installed: Option<String>, pub description: String, pub runtime: String,
}
impl Item {
    fn from_registry(v: &Value, installed: &[plugins::Plugin]) -> Self {
        let field = |key: &str| v[key].as_str().unwrap_or("").to_string();
        let id = field("id");
        Self { installed: installed.iter().find(|p| p.id == id).map(|p|p.version.clone()),
            id, name:field("name"), version:field("version"), description:field("description"),runtime:field("runtime") }
    }
}

#[derive(Clone, Debug)]
pub enum Change { Install(String), Remove(String), Update(String) }
impl Change {
    fn name(&self) -> &'static str { match self { Self::Install(_)=>"install",Self::Remove(_)=>"remove",Self::Update(_)=>"update"} }
    pub(crate) fn id(&self)->&str{match self{Self::Install(s)|Self::Remove(s)|Self::Update(s)=>s}}
}

#[derive(Debug)]
pub enum UiAction { None, Run(String) }

pub struct Browser {
    pub visible: bool,
    pub category: Category,
    pub query: String,
    pub selected: usize,
    pub offset: usize,
    pub note: String,
    pub busy: bool,
    pub confirm: Option<Change>,
    registry: Vec<Value>,
    loader: Option<Receiver<Result<Vec<Value>, String>>>,
    operation: Option<Receiver<Result<String, String>>>,
}
impl Browser {
    pub fn new() -> Self { Self {visible:false,category:Category::Discover,query:String::new(),selected:0,offset:0,
        note:"F5 refresh | type to search | Enter details/install | Esc close".into(),busy:false,confirm:None,
        registry:Vec::new(),loader:None,operation:None} }
    pub fn open(&mut self) {
        self.visible = true;
        if self.registry.is_empty() && self.loader.is_none() {self.refresh();}
    }
    pub fn close(&mut self) {self.visible=false;self.confirm=None;}
    pub fn installed(&mut self){self.open();self.category=Category::Installed;self.selected=0;self.offset=0;}
    pub fn refresh(&mut self){
        if self.busy {self.note="Complete the ongoing operation before refreshing".into();return;}
        if self.loader.is_some() {return;}
        let (tx,rx)=mpsc::channel();self.loader=Some(rx);
        self.note="Loading official registry from criteric/snack-plugins...".into();
        thread::spawn(move || {let result=marketplace::index().map_err(|e|e.to_string());let _=tx.send(result);});
    }
    pub fn poll(&mut self) -> bool {
        let mut changed=false;
        // Extract the result before changing an Option containing its Receiver.
        let loaded=self.loader.as_ref().map(|rx|rx.try_recv());
        match loaded {
            Some(Ok(Ok(entries)))=>{
                self.note=format!("Official registry: {} plugins",entries.len());
                self.registry=entries;self.selected=0;self.offset=0;
                self.loader=None;changed=true;
            },
            Some(Ok(Err(e)))=>{
                self.note=format!("Marketplace unavailable: {e} (installed plugins remain accessible)");
                self.loader=None;changed=true;
            },
            Some(Err(TryRecvError::Disconnected))=>{
                self.note="Marketplace request interrupted".into();self.loader=None;changed=true;
            },
            _=>{},
        }
        let done=self.operation.as_ref().map(|rx|rx.try_recv());
        match done {
            Some(Ok(Ok(info)))=>{
                self.note=info;self.busy=false;self.operation=None;changed=true;
            },
            Some(Ok(Err(e)))=>{
                self.note=format!("Plugin operation failed: {e}");
                self.busy=false;self.operation=None;changed=true;
            },
            Some(Err(TryRecvError::Disconnected))=>{
                self.note="Plugin operation interrupted".into();
                self.busy=false;self.operation=None;changed=true;
            },
            _=>{},
        }
        changed
    }
    pub fn is_loading(&self)->bool {self.loader.is_some()}
    pub fn items(&self)->Vec<Item>{
        let installed=plugins::load();
        let mut items:Vec<Item>=self.registry.iter().map(|x|Item::from_registry(x,&installed)).collect();
        if self.category==Category::Installed {
            for p in &installed {
                if !items.iter().any(|x|x.id==p.id){items.push(Item{id:p.id.clone(),name:p.name.clone(),version:"not indexed".into(),
                    installed:Some(p.version.clone()),description:"Locally installed plugin".into(),runtime:p.runtime.clone()});}
            }
            items.retain(|x|x.installed.is_some());
        }
        let query=self.query.to_lowercase();
        items.retain(|x|x.id.to_lowercase().contains(&query)||x.name.to_lowercase().contains(&query)||x.description.to_lowercase().contains(&query));
        items.sort_by(|a,b|a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        items
    }
    pub fn highlighted(&self)->Option<Item>{self.items().get(self.selected).cloned()}
    fn clamp(&mut self) {let n=self.items().len();self.selected=self.selected.min(n.saturating_sub(1));if n==0{self.offset=0;}}
    fn request(&mut self,change:Change){
        if self.busy {self.note="Another plugin operation is already running".into();return;}
        self.confirm=Some(change);
    }
    fn perform(&mut self,change:Change){
        let action=change.name();let name=change.id().to_string();
        self.note=format!("{action}: {name} ...");self.busy=true;
        let (tx,rx)=mpsc::channel();self.operation=Some(rx);
        thread::spawn(move || {
            let dir=plugins::directory();
            let result=match &change {
                Change::Install(id)=>marketplace::install_silent(id,&dir),
                Change::Update(id)=>marketplace::update_silent(id,&dir),
                Change::Remove(id)=> {
                    // Never allow a manifest ID to escape the plugin directory.
                    if id.is_empty() || !id.bytes().all(|b|b.is_ascii_alphanumeric()||b"._-".contains(&b)) {
                        Err(std::io::Error::other("Invalid plugin identifier"))
                    } else {fs::remove_file(dir.join(format!("{id}.yummy")))}
                }
            };
            let _=tx.send(result.map(|_|format!("Plugin {} completed: {}",action,change.id())).map_err(|e|e.to_string()));
        });
    }
    fn do_selected(&mut self,cmd:char)->UiAction{
        let Some(entry)=self.highlighted() else {self.note="Select a plugin first".into();return UiAction::None;};
        match cmd {
            'i'=>if entry.installed.is_none() {self.request(Change::Install(entry.id));} else {self.note="Already installed".into();},
            'x'=>if entry.installed.is_some(){self.request(Change::Remove(entry.id));}else{self.note="Not installed".into();},
            'u'=>if let Some(ref old)=entry.installed{
                match (semver::Version::parse(old),semver::Version::parse(&entry.version)) {
                    (Ok(a),Ok(b)) if b>a=>self.request(Change::Update(entry.id)),
                    _=>self.note="No newer official version available".into()
                }
            }else{self.note="Install the plugin first".into();},
            'p'=>if entry.installed.is_some(){self.close();return UiAction::Run(entry.id);} else {self.note="Install before running".into();},
            _=>{}
        }
        UiAction::None
    }
    pub fn key(&mut self,key:KeyEvent)->UiAction{
        if let Some(action)=self.confirm.take() {
            match key.code {
                KeyCode::Char('y')|KeyCode::Char('Y')=>self.perform(action),
                KeyCode::Esc|KeyCode::Char('n')|KeyCode::Char('N')=>self.note="Cancelled".into(),
                _=>self.confirm=Some(action),
            }
            return UiAction::None;
        }
        if key.code==KeyCode::Esc {self.close();return UiAction::None;}
        if key.code==KeyCode::F(5){self.refresh();return UiAction::None;}
        if key.code==KeyCode::Tab {
            self.category=if self.category==Category::Discover{Category::Installed}else{Category::Discover};
            self.selected=0;self.offset=0;return UiAction::None;
        }
        if self.busy {return UiAction::None;}
        match key.code {
            KeyCode::Up=>self.selected=self.selected.saturating_sub(1),
            KeyCode::Down=>self.selected=(self.selected+1).min(self.items().len().saturating_sub(1)),
            KeyCode::PageUp=>self.selected=self.selected.saturating_sub(8),
            KeyCode::PageDown=>self.selected=(self.selected+8).min(self.items().len().saturating_sub(1)),
            KeyCode::Home=>self.selected=0,
            KeyCode::End=>self.selected=self.items().len().saturating_sub(1),
            KeyCode::Backspace=>{self.query.pop();self.selected=0;self.offset=0;},
            KeyCode::Enter=>{
                if let Some(item)=self.highlighted(){
                    self.note=format!("{} ({}) — {} | press I to install, P to run, X to remove",item.name,item.id,item.description);
                }
            },
            KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)=> {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    match c.to_ascii_lowercase(){
                        'i'|'x'|'u'|'p'=>return self.do_selected(c.to_ascii_lowercase()),
                        _=>{}
                    }
                }
                // Use uppercase shortcuts to avoid preventing searches for words like "install".
                if c.is_ascii_uppercase() && "IXUP".contains(c) {return self.do_selected(c.to_ascii_lowercase());}
                if c=='/' {self.query.clear();self.selected=0;self.offset=0;} else {
                    self.query.push(c);self.selected=0;self.offset=0;
                }
            },
            _=>{}
        }
        self.clamp(); UiAction::None
    }
    pub fn mouse(&mut self,m:MouseEvent,w:u16,h:u16)->UiAction{
        if self.confirm.is_some(){return UiAction::None;}
        if m.kind==MouseEventKind::ScrollUp {self.selected=self.selected.saturating_sub(3);self.clamp();return UiAction::None;}
        if m.kind==MouseEventKind::ScrollDown {self.selected=(self.selected+3).min(self.items().len().saturating_sub(1));self.clamp();return UiAction::None;}
        if m.kind!=MouseEventKind::Down(MouseButton::Left){return UiAction::None;}
        if m.row==1 {
            if m.column<22 {self.category=Category::Discover;}else{self.category=Category::Installed;}
            self.selected=0;self.offset=0;return UiAction::None;
        }
        if m.row>=5 && m.row<h.saturating_sub(4) && m.column<w/2 {
            let idx=self.offset+(m.row-5) as usize;
            if idx<self.items().len(){self.selected=idx;}
        }
        if m.row==h.saturating_sub(3){
            let col=m.column;
            if col<14{return self.do_selected('i');}
            if col<26{return self.do_selected('u');}
            if col<38{return self.do_selected('x');}
            if col<47{return self.do_selected('p');}
            self.refresh();
        }
        UiAction::None
    }
    pub fn view_offset(&mut self,visible:usize){
        if self.selected<self.offset {self.offset=self.selected;}
        if visible>0 && self.selected>=self.offset+visible {self.offset=self.selected+1-visible;}
    }
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn filter_and_status(){
        let mut b=Browser::new();b.registry=vec![serde_json::json!({"id":"criteric.example","name":"Example","version":"0.1.0","runtime":"lua","description":"An example"})];
        b.query="example".into();assert_eq!(b.items().len(),1);b.query="missing".into();assert!(b.items().is_empty());
    }
    #[test] fn category_cycle(){let mut b=Browser::new();b.key(KeyEvent::new(KeyCode::Tab,KeyModifiers::NONE));assert_eq!(b.category,Category::Installed);}
}
