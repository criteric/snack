use std::{fs, io, path::PathBuf};

#[derive(Clone)]
pub struct Snapshot {
    pub lines: Vec<Vec<char>>,
    pub x: usize,
    pub y: usize,
}

pub struct Document {
    pub path: PathBuf,
    pub lines: Vec<Vec<char>>,
    pub x: usize,
    pub y: usize,
    pub top: usize,
    pub left: usize,
    pub dirty: bool,
    pub undo: Vec<Snapshot>,
    pub redo: Vec<Snapshot>,
    pub last_autosave: std::time::Instant,
    /// The anchor of a keyboard or mouse text selection, in (row, column) characters.
    pub selection_anchor: Option<(usize, usize)>,
}
impl Document {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let contents = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        let mut lines: Vec<Vec<char>> = contents.split('\n').map(|s| s.trim_end_matches('\r').chars().collect()).collect();
        if lines.is_empty() { lines.push(vec![]); }
        Ok(Self { path, lines, x: 0, y: 0, top: 0, left: 0, dirty: false, undo: vec![], redo: vec![], last_autosave: std::time::Instant::now(), selection_anchor: None })
    }
    pub fn title(&self) -> String {
        self.path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "untitled".into())
    }
    pub fn snapshot(&self) -> Snapshot { Snapshot { lines: self.lines.clone(), x: self.x, y: self.y } }
    pub fn checkpoint(&mut self) {
        if self.undo.len() >= 100 { self.undo.remove(0); }
        self.undo.push(self.snapshot());
        self.redo.clear();
        self.dirty = true;
    }
    fn restore(&mut self, s: Snapshot) { self.lines = s.lines; self.x = s.x; self.y = s.y; self.selection_anchor = None; self.dirty = true; }
    pub fn undo(&mut self) { if let Some(s) = self.undo.pop() { self.redo.push(self.snapshot()); self.restore(s); } }
    pub fn redo(&mut self) { if let Some(s) = self.redo.pop() { self.undo.push(self.snapshot()); self.restore(s); } }
    pub fn insert(&mut self, c: char) { self.checkpoint(); self.selection_anchor=None; self.lines[self.y].insert(self.x, c); self.x += 1; }
    pub fn newline(&mut self) {
        self.checkpoint();self.selection_anchor=None;
        let indent: Vec<char> = self.lines[self.y].iter().take_while(|c| **c==' '||**c=='\t').copied().collect();
        let right=self.lines[self.y].split_off(self.x);
        self.y+=1;let mut line=indent;
        let len=line.len();line.extend(right);self.lines.insert(self.y,line);self.x=len;
    }
    pub fn insert_smart(&mut self, c:char) {
        let closing=match c {'('=>Some(')'), '['=>Some(']'), '{'=>Some('}'), _=>None};
        if let Some(close)=closing {
            let at_end=self.x==self.lines[self.y].len();
            if at_end || self.lines[self.y][self.x].is_whitespace(){
                self.checkpoint();self.selection_anchor=None;self.lines[self.y].insert(self.x,c);
                self.lines[self.y].insert(self.x+1,close);self.x+=1;return;
            }
        }
        // Skip an automatically inserted closing character rather than doubling it.
        if matches!(c,')'|']'|'}') && self.lines[self.y].get(self.x)==Some(&c) {self.x+=1;return;}
        self.insert(c);
    }
    pub fn replace_all(&mut self,needle:&str,replacement:&str)->usize{
        if needle.is_empty(){return 0;}
        let original=self.text();let count=original.matches(needle).count();
        if count==0{return 0;}self.checkpoint();self.selection_anchor=None;
        let edited=original.replace(needle,replacement);
        self.lines=edited.split('\n').map(|s|s.chars().collect()).collect();
        self.y=self.y.min(self.lines.len()-1);self.x=self.x.min(self.lines[self.y].len());count
    }
    /// Replace the typed abbreviation immediately before the cursor in one undo step.
    /// cursor_offset counts Unicode chars in replacement (including newline chars).
    pub fn replace_before_cursor(&mut self, typed_chars:usize, replacement:&str, cursor_offset:usize) {
        if typed_chars > self.x { return; }
        self.checkpoint();self.selection_anchor=None;
        let start = self.x - typed_chars;
        let trailing = self.lines[self.y].split_off(self.x);
        self.lines[self.y].truncate(start);
        let pieces:Vec<&str> = replacement.split('\n').collect();
        self.lines[self.y].extend(pieces[0].chars());
        let base_y=self.y;
        for (n,piece) in pieces.iter().enumerate().skip(1) {
            self.lines.insert(base_y+n,piece.chars().collect());
        }
        let end_y=base_y+pieces.len()-1;
        self.lines[end_y].extend(trailing);
        self.y=base_y;
        self.x=start;
        for c in replacement.chars().take(cursor_offset) {
            if c=='\n' { self.y+=1;self.x=0; }
            else { self.x+=1; }
        }
    }
    pub fn backspace(&mut self) {
        self.selection_anchor=None;
        if self.x > 0 { self.checkpoint(); self.x -= 1; self.lines[self.y].remove(self.x); }
        else if self.y > 0 { self.checkpoint(); let right = self.lines.remove(self.y); self.y -= 1; self.x = self.lines[self.y].len(); self.lines[self.y].extend(right); }
    }
    pub fn delete(&mut self) {
        self.selection_anchor=None;
        if self.x < self.lines[self.y].len() { self.checkpoint(); self.lines[self.y].remove(self.x); }
        else if self.y + 1 < self.lines.len() { self.checkpoint(); let next = self.lines.remove(self.y + 1); self.lines[self.y].extend(next); }
    }
    pub fn movement(&mut self, key: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        match key {
            KeyCode::Up => self.y = self.y.saturating_sub(1),
            KeyCode::Down => self.y = (self.y + 1).min(self.lines.len()-1),
            KeyCode::Left => { if self.x > 0 { self.x -= 1; } else if self.y > 0 { self.y -= 1; self.x = self.lines[self.y].len(); } },
            KeyCode::Right => { if self.x < self.lines[self.y].len() { self.x += 1; } else if self.y + 1 < self.lines.len() { self.y += 1; self.x = 0; } },
            KeyCode::Home => self.x = 0,
            KeyCode::End => self.x = self.lines[self.y].len(),
            KeyCode::PageUp => self.y = self.y.saturating_sub(15),
            KeyCode::PageDown => self.y = (self.y + 15).min(self.lines.len()-1),
            _ => {}
        }
        self.x = self.x.min(self.lines[self.y].len());
    }
    /// Character-indexed, half-open selection. Mouse and keyboard share the same model.
    pub fn selection_range(&self) -> Option<((usize,usize),(usize,usize))> {
        let a = self.selection_anchor?;
        let b = (self.y,self.x);
        if a==b {None} else if a<b {Some((a,b))} else {Some((b,a))}
    }
    pub fn is_selected(&self,row:usize,col:usize)->bool {
        self.selection_range().is_some_and(|(start,end)| (row,col)>=start && (row,col)<end)
    }
    pub fn selected_text(&self)->Option<String> {
        let ((sy,sx),(ey,ex))=self.selection_range()?;
        if sy==ey {return Some(self.lines[sy][sx..ex].iter().collect());}
        let mut result:String=self.lines[sy][sx..].iter().collect();
        for row in sy+1..ey {result.push('\n');result.extend(self.lines[row].iter().copied());}
        result.push('\n');result.extend(self.lines[ey][..ex].iter().copied());Some(result)
    }
    pub fn select_all(&mut self){
        self.selection_anchor=Some((0,0));
        self.y=self.lines.len()-1;self.x=self.lines[self.y].len();
    }
    pub fn select_line(&mut self){
        self.selection_anchor=Some((self.y,0));
        if self.y+1<self.lines.len() {self.y+=1;self.x=0;}
        else {self.x=self.lines[self.y].len();}
    }
    /// Replaces selected text (or inserts at cursor) with a single undo checkpoint.
    pub fn replace_selection(&mut self,new_text:&str){
        self.checkpoint();
        let ((sy,sx),(ey,ex))=self.selection_range().unwrap_or(((self.y,self.x),(self.y,self.x)));
        let trailing:Vec<char>=self.lines[ey][ex..].to_vec();
        self.lines[sy].truncate(sx);
        if ey>sy {self.lines.drain(sy+1..=ey);}
        let parts:Vec<&str>=new_text.split('\n').collect();
        self.lines[sy].extend(parts[0].chars());
        for (i,part) in parts.iter().enumerate().skip(1) {
            self.lines.insert(sy+i,part.chars().collect());
        }
        self.y=sy+parts.len()-1;
        self.x=parts.last().map(|s|s.chars().count()).unwrap_or(0);
        if self.y==sy {self.x+=sx;}
        self.lines[self.y].extend(trailing);
        self.selection_anchor=None;
    }
    pub fn delete_selection(&mut self)->bool {
        if self.selection_range().is_none(){return false;}
        self.replace_selection("");true
    }
    pub fn copy_selection_or_line(&self)->String {
        self.selected_text().unwrap_or_else(||format!("{}\n",self.lines[self.y].iter().collect::<String>()))
    }
    pub fn delete_line(&mut self){
        self.checkpoint();self.selection_anchor=None;
        if self.lines.len()>1 {self.lines.remove(self.y);self.y=self.y.min(self.lines.len()-1);}
        else {self.lines[0].clear();}
        self.x=0;
    }
    pub fn duplicate_line(&mut self){
        self.checkpoint();self.selection_anchor=None;
        let copied=self.lines[self.y].clone();self.y+=1;
        self.lines.insert(self.y,copied);
        self.x=self.x.min(self.lines[self.y].len());
    }
    pub fn toggle_line_comment(&mut self){
        let ext=self.path.extension().and_then(|s|s.to_str()).unwrap_or("");
        let (open,close)=match ext {
            "html"|"htm"|"xml"|"svg" => ("<!-- "," -->"),
            "py"|"sh"|"bash"|"yaml"|"yml"|"toml"|"rb" => ("# ",""),
            "css" => ("/* "," */"),
            _ => ("// ",""),
        };
        let (first,last)=if let Some(((sy,_),(ey,ex)))=self.selection_range(){
            (sy,if ex==0 && ey>sy{ey-1}else{ey})
        }else{(self.y,self.y)};
        self.checkpoint();
        for i in first..=last {
            let chars=&mut self.lines[i];
            let lead=chars.iter().take_while(|c|c.is_whitespace()).count();
            let open_chars:Vec<char>=open.chars().collect();
            let close_chars:Vec<char>=close.chars().collect();
            let uncomment=chars[lead..].starts_with(&open_chars)
                && (close.is_empty() || chars.ends_with(&close_chars));
            if uncomment {
                chars.drain(lead..lead+open.chars().count());
                if !close.is_empty(){let len=chars.len();chars.drain(len-close.chars().count()..);}
            }else{
                for (n,c) in open.chars().enumerate(){chars.insert(lead+n,c);}
                chars.extend(close.chars());
            }
        }
        self.x=self.x.min(self.lines[self.y].len());self.selection_anchor=None;
    }
    pub fn text(&self) -> String { self.lines.iter().map(|l| l.iter().collect::<String>()).collect::<Vec<_>>().join("\n") }
    pub fn save(&mut self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() { if !parent.as_os_str().is_empty() { fs::create_dir_all(parent)?; } }
        fs::write(&self.path, self.text())?;
        self.dirty = false;
        Ok(())
    }
    pub fn recovery_path(&self) -> PathBuf {
        let mut p = self.path.as_os_str().to_os_string();
        p.push(".snack-recovery");
        PathBuf::from(p)
    }
    pub fn autosave_recovery(&mut self) -> io::Result<()> {
        if !self.dirty { return Ok(()); }
        fs::write(self.recovery_path(), self.text())?;
        self.last_autosave = std::time::Instant::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replace_snippet_is_one_undo_step() {
        let mut d=Document::open(std::env::temp_dir().join("snack-completion-placeholder-1546.html")).unwrap();
        d.insert('!');
        d.replace_before_cursor(1,"<html>\n  hi\n</html>",9);
        assert!(d.text().starts_with("<html>"));
        assert_eq!(d.y,1);
        d.undo();assert_eq!(d.text(),"!");
    }
    #[test] fn smart_pairs_and_indent(){
        let mut d=Document::open(std::env::temp_dir().join("snack-test-smart.txt")).unwrap();
        d.insert_smart('{');assert_eq!(d.text(),"{}");d.newline();assert_eq!(d.y,1);
        d.insert_smart('a');d.insert_smart(')');assert!(d.text().contains('a'));
    }
    #[test] fn replace_all_once(){
        let mut d=Document::open(std::env::temp_dir().join("snack-test-replace.txt")).unwrap();
        for c in "one one".chars(){d.insert(c);}assert_eq!(d.replace_all("one","two"),2);
        d.undo();assert_eq!(d.text(),"one one");
    }
    #[test]
    fn edit_and_undo() {
        let mut d = Document::open(std::env::temp_dir().join("snack-does-not-exist-test.txt")).unwrap();
        d.insert('a'); d.newline(); d.insert('b');
        assert_eq!(d.text(), "a\nb");
        d.undo(); assert_eq!(d.text(), "a\n");
        d.redo(); assert_eq!(d.text(), "a\nb");
    }
}

#[cfg(test)]
mod context_edit_tests {
    use super::*;
    fn d()->Document { Document::open(std::env::temp_dir().join("snack-context-test-nonexistent-43872.txt")).unwrap() }
    #[test] fn selection_copy_delete_and_undo(){
        let mut doc=d();doc.replace_selection("hello\nworld");doc.selection_anchor=Some((0,2));doc.y=1;doc.x=3;
        assert_eq!(doc.selected_text().as_deref(),Some("llo\nwor"));
        doc.replace_selection("X");assert_eq!(doc.text(),"heXld");doc.undo();assert_eq!(doc.text(),"hello\nworld");
    }
    #[test] fn select_all_replace_and_duplicate(){
        let mut doc=d();doc.replace_selection("one\ntwo");doc.select_all();assert_eq!(doc.selected_text().as_deref(),Some("one\ntwo"));
        doc.replace_selection("hi");assert_eq!(doc.text(),"hi");doc.duplicate_line();assert_eq!(doc.text(),"hi\nhi");
    }
    #[test] fn comment_toggle_roundtrip(){
        let mut doc=d();doc.replace_selection("  let x = 1;");doc.toggle_line_comment();
        assert_eq!(doc.text(),"  // let x = 1;");doc.toggle_line_comment();assert_eq!(doc.text(),"  let x = 1;");
    }
}
