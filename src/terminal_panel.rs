//! An editor-integrated command/output panel. This is NOT a full PTY terminal.
//! Ctrl+` or View > Terminal Panel opens it. Ctrl+T keeps the external interactive shell.
use std::{path::PathBuf,process::Command,sync::mpsc::{self,Sender,Receiver},thread};

pub struct Panel {pub visible:bool,pub focus:bool,pub input:String,pub lines:Vec<String>,pub running:bool, sender:Sender<(String,String)>,receiver:Receiver<(String,String)>}
impl Panel {
    pub fn new()->Self {let (sender,receiver)=mpsc::channel();Self{visible:false,focus:false,input:String::new(),lines:vec!["Snack command runner — type a shell command and press Enter".into()],running:false,sender,receiver}}
    pub fn toggle(&mut self){self.visible=!self.visible;self.focus=self.visible;}
    pub fn execute(&mut self,root:PathBuf){
        let command=std::mem::take(&mut self.input);if command.trim().is_empty(){return;}
        if self.running{self.lines.push("A command is already running".into());return;}
        self.running=true;self.lines.push(format!("> {command}"));let tx=self.sender.clone();
        thread::spawn(move ||{
            let mut cmd=if cfg!(windows){let mut p=Command::new("cmd");p.arg("/C").arg(&command);p}
                else {let mut p=Command::new("sh");p.arg("-c").arg(&command);p};
            let result=cmd.current_dir(root).output();
            let (text,status)=match result {
                Ok(out)=>{
                    let mut output=String::from_utf8_lossy(&out.stdout).to_string();
                    output.push_str(&String::from_utf8_lossy(&out.stderr));
                    (output,format!("Process exited: {}",out.status))
                },Err(e)=>(e.to_string(),"Command failed to launch".into()),
            };
            let _=tx.send((text,status));
        });
    }
    pub fn poll(&mut self)->bool{
        let mut changed=false;while let Ok((text,status))=self.receiver.try_recv(){
            for line in text.lines().take(200){self.lines.push(line.to_owned());}
            self.lines.push(status);self.running=false;changed=true;
        }
        if self.lines.len()>300{self.lines.drain(..self.lines.len()-300);}changed
    }
}
