//! Small, asynchronous Language Server Protocol client.
//! Launches an installed language server on demand; no LSP process runs on the UI thread.
//! Supports completion + publishDiagnostics. Full code actions, hover and DAP are future work.
use std::{io::{self,BufRead,BufReader,Read,Write},path::{Path,PathBuf},process::{Child,ChildStdin,ChildStdout,Command,Stdio},sync::mpsc::{self,Receiver,Sender},thread};
use serde_json::{Value,json};

pub struct Request {pub path:PathBuf,pub root:PathBuf,pub text:String,pub line:usize,pub column:usize,pub prefix:String}
#[derive(Debug)] pub enum Reply {Completion {path:PathBuf,items:Vec<Suggestion>},Diagnostics {path:PathBuf,items:Vec<String>},Message(String)}
#[derive(Debug)] pub struct Suggestion {pub label:String,pub insert:String,pub detail:String}
pub struct Client {pub sender:Sender<Request>,pub results:Receiver<Reply>}
impl Client {
    pub fn new()->Self {
        let (sender,requests)=mpsc::channel::<Request>();let (results,receiver)=mpsc::channel::<Reply>();
        thread::spawn(move || worker(requests,results));Self{sender,results:receiver}
    }
}
fn server_spec(path:&Path)->Option<(&'static str,&'static [&'static str],&'static str)> {
    match path.extension().and_then(|x|x.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "rs"=>Some(("rust-analyzer",&[],"rust")),
        "py"=>Some(("pylsp",&[],"python")),
        "js"|"jsx"|"ts"|"tsx"=>Some(("typescript-language-server",&["--stdio"],"typescript")),
        "c"|"h"|"cpp"|"hpp"|"cc"=>Some(("clangd",&[],"cpp")),
        "go"=>Some(("gopls",&[],"go")),
        "html"|"htm"=>Some(("vscode-html-language-server",&["--stdio"],"html")),
        "css"=>Some(("vscode-css-language-server",&["--stdio"],"css")),
        _=>None
    }
}
fn uri(path:&Path)->String {
    // File URI encoding for spaces and non-ASCII paths. On Windows, prepend / before drive.
    let raw=path.canonicalize().unwrap_or_else(|_|path.to_path_buf()).to_string_lossy().replace('\\',"/");
    let raw=if cfg!(windows) && !raw.starts_with('/') {format!("/{raw}")} else {raw};
    let mut encoded=String::new();for b in raw.bytes(){if b.is_ascii_alphanumeric()||b"/-._~".contains(&b){encoded.push(b as char);}else{encoded.push_str(&format!("%{b:02X}"));}}
    format!("file://{encoded}")
}
struct Server { child:Child,stdin:ChildStdin,stdout:BufReader<ChildStdout>,lang:String,opened:Option<String>,id:u64,version:u64 }
impl Server {
    fn start(req:&Request,name:&str,args:&[&str],lang:&str)->io::Result<Self>{
        let mut child=Command::new(name).args(args).current_dir(&req.root)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
        let stdin=child.stdin.take().ok_or_else(||io::Error::other("LSP stdin unavailable"))?;
        let stdout=BufReader::new(child.stdout.take().ok_or_else(||io::Error::other("LSP stdout unavailable"))?);
        let mut server=Self{child,stdin,stdout,lang:lang.into(),opened:None,id:1,version:0};
        server.send(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "processId":std::process::id(),"rootUri":uri(&req.root),"capabilities":{
                "textDocument":{"completion":{"completionItem":{"snippetSupport":false}},"publishDiagnostics":{}}},
            "clientInfo":{"name":"Snack","version":env!("CARGO_PKG_VERSION")}}}))?;
        loop {let v=server.read()?;
            if v.get("id")==Some(&json!(1)) {if v.get("error").is_some(){return Err(io::Error::other("LSP initialize rejected"));}break;}
            server.reply_to_server(&v)?;
        }
        server.send(&json!({"jsonrpc":"2.0","method":"initialized","params":{}}))?;
        Ok(server)
    }
    fn send(&mut self,v:&Value)->io::Result<()> {let data=serde_json::to_vec(v).map_err(io::Error::other)?;write!(self.stdin,"Content-Length: {}\r\n\r\n",data.len())?;self.stdin.write_all(&data)?;self.stdin.flush()}
    fn read(&mut self)->io::Result<Value> {
        let mut len=0;loop {let mut line=String::new();if self.stdout.read_line(&mut line)?==0{return Err(io::Error::new(io::ErrorKind::UnexpectedEof,"language server disconnected"));}
            if line=="\r\n"||line=="\n"{break;}
            let lower=line.to_ascii_lowercase();
            if let Some(value)=lower.strip_prefix("content-length:") {len=value.trim().parse::<usize>().unwrap_or(0);}
        }
        if len==0||len>8_000_000{return Err(io::Error::other("invalid LSP Content-Length"));}
        let mut buf=vec![0;len];self.stdout.read_exact(&mut buf)?;Ok(serde_json::from_slice(&buf).map_err(io::Error::other)?)
    }
    fn reply_to_server(&mut self,v:&Value)->io::Result<()> {
        if v.get("method").is_some() && v.get("id").is_some(){
            self.send(&json!({"jsonrpc":"2.0","id":v["id"],"result":null}))?;
        }Ok(())
    }
    fn completion(&mut self,req:&Request,tx:&Sender<Reply>)->io::Result<()> {
        let current=uri(&req.path);
        if self.opened.as_ref()!=Some(&current) {
            if let Some(old)=self.opened.take(){self.send(&json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":old}}}))?;}
            self.version=1;
            self.send(&json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":current,"languageId":self.lang,"version":self.version,"text":req.text}}}))?;
            self.opened=Some(current.clone());
        }else{
            self.version+=1;
            self.send(&json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":current,"version":self.version},"contentChanges":[{"text":req.text}]}}))?;
        }
        self.id+=1;let id=self.id;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":"textDocument/completion","params":{
            "textDocument":{"uri":current},"position":{"line":req.line,"character":req.column},"context":{"triggerKind":1}}}))?;
        loop {
            let response=self.read()?;
            if response.get("id")==Some(&json!(id)) {
                let raw=&response["result"];
                let items=if let Some(a)=raw.as_array(){Some(a)}else{raw.get("items").and_then(Value::as_array)};
                let parsed=items.into_iter().flatten().take(100).filter_map(|v|{
                    let label=v.get("label")?.as_str()?.to_owned();
                    let insert=v.get("textEdit").and_then(|x|x.get("newText")).and_then(Value::as_str)
                        .or_else(||v.get("insertText").and_then(Value::as_str)).unwrap_or(&label).to_owned();
                    if v.get("insertTextFormat").and_then(Value::as_u64)==Some(2){return None;} // Snippet text edits need a richer client.
                    let detail=v.get("detail").and_then(Value::as_str).unwrap_or("LSP").to_owned();
                    if !req.prefix.is_empty() && !label.to_ascii_lowercase().starts_with(&req.prefix.to_ascii_lowercase()){return None;}
                    Some(Suggestion{label,insert,detail})
                }).collect();
                let _=tx.send(Reply::Completion{path:req.path.clone(),items:parsed});return Ok(());
            }
            if response.get("method").and_then(Value::as_str)==Some("textDocument/publishDiagnostics") {
                if response["params"]["uri"].as_str()==Some(current.as_str()) {
                    let items=response["params"]["diagnostics"].as_array().into_iter().flatten().take(100).filter_map(|d|{
                        let message=d["message"].as_str()?;
                        let line=d["range"]["start"]["line"].as_u64().unwrap_or(0)+1;
                        Some(format!("{line}: {message}"))
                    }).collect();let _=tx.send(Reply::Diagnostics{path:req.path.clone(),items});
                }
            }
            self.reply_to_server(&response)?;
        }
    }
}
impl Drop for Server {fn drop(&mut self){let _=self.child.kill();let _=self.child.wait();}}
fn worker(rx:Receiver<Request>,tx:Sender<Reply>) {
    let mut server:Option<Server>=None;
    while let Ok(req)=rx.recv(){
        let Some((name,args,lang))=server_spec(&req.path) else{let _=tx.send(Reply::Message(format!("No LSP configured for {}",req.path.display())));continue;};
        if server.as_ref().is_some_and(|s|s.lang!=lang){server=None;}
        if server.is_none(){match Server::start(&req,name,args,lang){
            Ok(s)=>{server=Some(s);let _=tx.send(Reply::Message(format!("LSP: {name} connected")));}
            Err(e)=>{let _=tx.send(Reply::Message(format!("LSP: {name} unavailable ({e})")));continue;}
        }}
        if let Some(s)=server.as_mut(){if let Err(e)=s.completion(&req,&tx){let _=tx.send(Reply::Message(format!("LSP error: {e}")));server=None;}}
    }
}
#[cfg(test)] mod tests {use super::*;#[test]fn encoded_uri(){let u=uri(Path::new("/tmp/hello world.rs"));assert!(u.contains("%20"));}}
