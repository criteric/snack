use std::{env, io::{self, Write}, process::{Command,Stdio}};
use crate::config::Config;
const KEYS: &[&str] = &["autosave_seconds","show_tree","vim_mode","tab_width","autocomplete"];

pub fn open(mode: Option<&str>) -> io::Result<()> {
    let gui=match mode {
        Some("--tui")=>false,
        Some("--gui")=>true,
        _=>cfg!(windows)||env::var_os("DISPLAY").is_some()||env::var_os("WAYLAND_DISPLAY").is_some(),
    };
    if gui && cfg!(windows) {return windows_settings();}
    if gui && Command::new("zenity").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s|s.success()) {
        gtk()
    }else if mode==Some("--gui"){
        Err(io::Error::new(io::ErrorKind::NotFound,"GTK settings require zenity and a desktop session"))
    }else{tui()}
}
fn gtk()->io::Result<()> {
    let mut c=Config::load();
    loop {
        let mut cmd=Command::new("zenity");
        cmd.args(["--list","--title=Snack Settings","--width=520","--height=300","--text=Choose a setting to edit","--column=Setting","--column=Value"]);
        for k in KEYS {cmd.arg(k).arg(c.get(k));}
        let output=cmd.output()?;
        if !output.status.success(){break;}
        let key=String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !KEYS.contains(&key.as_str()){continue;}
        let value=Command::new("zenity").args(["--entry","--title=Edit Snack setting"])
            .arg(format!("--text=Enter new value for {key}"))
            .arg(format!("--entry-text={}",c.get(&key))).output()?;
        if !value.status.success(){continue;}
        let new=String::from_utf8_lossy(&value.stdout).trim().to_owned();
        match c.set(&key,&new){
            Ok(())=>c.save()?,
            Err(e)=>{let _=Command::new("zenity").args(["--error","--title=Snack Settings"]).arg(format!("--text={e}")).status();},
        }
    }
    Ok(())
}
fn tui()->io::Result<()> {
    let mut c=Config::load();
    loop {
        println!("\nSnack Settings (TUI)\n0) Save and exit");
        for (i,k) in KEYS.iter().enumerate(){println!("{}) {} = {}",i+1,k,c.get(k));}
        print!("Choose setting: ");io::stdout().flush()?;
        let mut line=String::new();if io::stdin().read_line(&mut line)?==0{break;}
        let n=line.trim().parse::<usize>().unwrap_or(0);
        if n==0{break;}
        let Some(&key)=KEYS.get(n-1) else{println!("Unknown selection");continue;};
        print!("New value for {key}: ");io::stdout().flush()?;
        line.clear();if io::stdin().read_line(&mut line)?==0{break;}
        if let Err(e)=c.set(key,line.trim()){println!("Invalid setting: {e}");}
        else{c.save()?;println!("Saved");}
    }
    Ok(())
}

#[cfg(windows)]
fn windows_settings()->io::Result<()> {
    let mut cfg=Config::load();
    for key in KEYS {
        let script=r#"Add-Type -AssemblyName Microsoft.VisualBasic
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$answer=[Microsoft.VisualBasic.Interaction]::InputBox(('Enter value for ' + $env:SNACK_KEY), 'Snack Settings', $env:SNACK_VALUE)
if ($answer -ne '') { [Console]::WriteLine($answer) }
"#;
        let output=Command::new("powershell.exe").args(["-NoProfile","-STA","-NonInteractive","-Command",script])
            .env("SNACK_KEY",key).env("SNACK_VALUE",cfg.get(key)).output()?;
        if !output.status.success(){return Err(io::Error::other(String::from_utf8_lossy(&output.stderr).to_string()));}
        let answer=String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if answer.is_empty(){continue;}
        if let Err(e)=cfg.set(key,&answer){eprintln!("Invalid {key}: {e}");}
        else{cfg.save()?;}
    }
    Ok(())
}
#[cfg(not(windows))]
fn windows_settings()->io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported,"Windows GUI only"))
}
