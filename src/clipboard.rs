//! Native system clipboard where available, plus an editor-local fallback in App.
//! No shell interpolation of clipboard contents (pipes are supplied as process stdin).
use std::{io::{self,Write},process::{Command,Stdio}};

fn write_with(program:&str,args:&[&str],text:&str)->io::Result<()> {
    let mut child=Command::new(program).args(args).stdin(Stdio::piped())
        .stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
    if let Some(mut input)=child.stdin.take(){input.write_all(text.as_bytes())?;}
    let status=child.wait()?;
    if status.success(){Ok(())}else{Err(io::Error::other(format!("{program} failed")))}
}
fn read_with(program:&str,args:&[&str])->io::Result<String> {
    let output=Command::new(program).args(args).output()?;
    if !output.status.success(){return Err(io::Error::other(format!("{program} failed")));}
    String::from_utf8(output.stdout).map_err(io::Error::other)
}
pub fn write(text:&str)->io::Result<()> {
    #[cfg(target_os="windows")]
    { return write_with("powershell.exe", &["-NoProfile","-Command","Set-Clipboard -Value ([Console]::In.ReadToEnd())"],text); }
    #[cfg(target_os="macos")]
    { return write_with("pbcopy", &[],text); }
    #[cfg(all(not(target_os="windows"),not(target_os="macos")))]
    {
        let candidates:&[(&str,&[&str])]=&[
            ("wl-copy",&[]),("xclip", &["-selection","clipboard"]),
            ("xsel", &["--clipboard","--input"]),
        ];
        for (prog,args) in candidates {if write_with(prog,args,text).is_ok(){return Ok(());}}
        Err(io::Error::new(io::ErrorKind::NotFound,"Install wl-clipboard or xclip for cross-app clipboard"))
    }
}
pub fn read()->io::Result<String>{
    #[cfg(target_os="windows")]
    { return read_with("powershell.exe", &["-NoProfile","-Command","Get-Clipboard -Raw"]); }
    #[cfg(target_os="macos")]
    { return read_with("pbpaste", &[]); }
    #[cfg(all(not(target_os="windows"),not(target_os="macos")))]
    {
        let candidates:&[(&str,&[&str])]=&[
            ("wl-paste",&["--no-newline"]),("xclip",&["-selection","clipboard","-o"]),
            ("xsel",&["--clipboard","--output"]),
        ];
        for (prog,args) in candidates {if let Ok(text)=read_with(prog,args){return Ok(text);}}
        Err(io::Error::new(io::ErrorKind::NotFound,"No supported desktop clipboard"))
    }
}
