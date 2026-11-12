//! OS-native file/folder selection. Linux uses zenity; Windows uses WinForms.
use std::{io, path::{Path,PathBuf}, process::Command};
#[derive(Clone,Copy)]
pub enum Picker { File, Folder, SaveAs }

#[cfg(windows)]
pub fn pick(kind:Picker,cwd:&Path,filename:&Path)->io::Result<Option<PathBuf>> {
    // No user-controlled path is interpolated into the PowerShell source.
    let script=r#"
Add-Type -AssemblyName System.Windows.Forms
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$start = $env:SNACK_PICKER_DIR
switch ($env:SNACK_PICKER_KIND) {
    'folder' {
        $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
        $dialog.Description = 'Snack - Open Folder'
        if (Test-Path -LiteralPath $start -PathType Container) { $dialog.SelectedPath = $start }
        if ($dialog.ShowDialog() -eq 'OK') { [Console]::WriteLine($dialog.SelectedPath) }
    }
    'save' {
        $dialog = New-Object System.Windows.Forms.SaveFileDialog
        $dialog.Title = 'Snack - Save As'
        $dialog.InitialDirectory = $start
        $dialog.FileName = [IO.Path]::GetFileName($env:SNACK_PICKER_FILE)
        $dialog.OverwritePrompt = $true
        if ($dialog.ShowDialog() -eq 'OK') { [Console]::WriteLine($dialog.FileName) }
    }
    default {
        $dialog = New-Object System.Windows.Forms.OpenFileDialog
        $dialog.Title = 'Snack - Open File'
        $dialog.InitialDirectory = $start
        if ($dialog.ShowDialog() -eq 'OK') { [Console]::WriteLine($dialog.FileName) }
    }
}
"#;
    let kind=match kind {Picker::Folder=>"folder",Picker::File=>"file",Picker::SaveAs=>"save"};
    let output=Command::new("powershell.exe")
        .args(["-NoProfile","-STA","-NonInteractive","-Command",script])
        .env("SNACK_PICKER_DIR",cwd)
        .env("SNACK_PICKER_FILE",filename)
        .env("SNACK_PICKER_KIND",kind).output()?;
    if !output.status.success(){return Err(io::Error::other(format!("Windows picker failed: {}",String::from_utf8_lossy(&output.stderr))));}
    let value=String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(if value.is_empty(){None}else{Some(PathBuf::from(value))})
}

#[cfg(not(windows))]
pub fn pick(kind:Picker,cwd:&Path,filename:&Path)->io::Result<Option<PathBuf>> {
    let mut cmd=Command::new("zenity");
    cmd.arg("--file-selection");
    match kind {
        Picker::File => {cmd.arg("--title=Snack - Open File")
            .arg(format!("--filename={}/",cwd.display()));}
        Picker::Folder => {cmd.arg("--directory").arg("--title=Snack - Open Folder")
            .arg(format!("--filename={}/",cwd.display()));}
        Picker::SaveAs => {cmd.arg("--save").arg("--confirm-overwrite")
            .arg("--title=Snack - Save As")
            .arg(format!("--filename={}",filename.display()));}
    }
    let result=cmd.current_dir(cwd).output()?;
    if result.status.success(){
        let name=String::from_utf8_lossy(&result.stdout).trim_end_matches(['\r','\n']).to_string();
        Ok(if name.is_empty(){None}else{Some(PathBuf::from(name))})
    } else if result.status.code()==Some(1){ Ok(None) }
    else {Err(io::Error::other(format!("GTK picker failed: {}",String::from_utf8_lossy(&result.stderr))))}
}
