#[path = "../package.rs"]
mod package;
#[path = "../platform.rs"]
mod platform;
#[path = "../marketplace.rs"]
mod marketplace;

use std::{env, fs, io::{self, IsTerminal, Write}, path::{Path, PathBuf}, process::{Command, Stdio}, time::{SystemTime, UNIX_EPOCH}};

fn err(message: impl Into<String>) -> io::Error { io::Error::new(io::ErrorKind::Other, message.into()) }
fn usage() {
    println!("Snack CLI (marketplace preview)\n\
      Usage: snackctl <command> [arguments]\n\
      new [name] [--language lua|rust] [--runtime lua|native|wasm]\n\
      compiler [project]      Backup source, compile supported runtime, package .yummy\n\
      build [project]         Package project as .yummy without backup\n\
      validate <file.yummy|project-dir>\n\
      unzip -ex <file.yummy> [--output DIR]\n\
      install <file.yummy|plugin-id>  Install local or official registry package\n\
      uninstall <plugin-id>  Remove installed archive\n\
      list                   List installed archives\n\
      doctor                 Check required tools\n\
      dev [project]          Run a development build (no hot reload yet)\n\
      search <term>          Search official registry AND GitHub community\n\
      publish <file.yummy> [--dry-run]       Publish to criteric/snack-plugins\n\
      publish <file.yummy> --github --repo owner/name  External release\n\
      update                 Notify about official plugin updates\n\
      settings               Open Snack settings via snack settings\n\
      NOTE: Lua plugins can be executed on demand in Snack 0.6 after consent. Native/WASM execution and arbitrary language translation remain unsupported.\n");
}
fn flag(args:&[String], key:&str)->Option<String>{args.windows(2).find(|p|p[0]==key).map(|p|p[1].clone())}
fn positional(args:&[String])->Option<&str> {
    let mut skip=false;
    for (i,value) in args.iter().enumerate() {
        if skip {skip=false;continue;}
        if ["--language","--runtime","--repo","--output","--platform"].contains(&value.as_str()) {skip=true;continue;}
        if value.starts_with('-'){continue;}
        return Some(args[i].as_str());
    }
    None
}
fn prompt(label: &str, default: &str) -> io::Result<String> {
    if !io::stdin().is_terminal() { return Ok(default.into()); }
    print!("{label} [{default}]: "); io::stdout().flush()?;
    let mut answer=String::new(); io::stdin().read_line(&mut answer)?;
    let answer=answer.trim(); Ok(if answer.is_empty() { default.into() } else {answer.into()})
}
fn home_data() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(env::var_os("APPDATA").or_else(||env::var_os("USERPROFILE")).unwrap_or_default()).join("snack/plugins")
    } else {
        env::var_os("XDG_DATA_HOME").map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".local/share"))
            .join("snack/plugins")
    }
}
fn path_arg(args:&[String]) -> PathBuf {
    positional(args).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}
fn project_manifest(dir:&Path) -> io::Result<std::collections::HashMap<String,String>> {
    let source=fs::read_to_string(dir.join("manifest.toml"))?;
    package::manifest_fields(&source)
}
fn clean_id(s:&str) -> String {
    s.to_ascii_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c=='_' || c=='-' {c} else {'-'}).collect()
}
fn new(args:&[String]) -> io::Result<()> {
    let name=match positional(args){Some(x)=>x.into(),None=>prompt("Plugin name", "my-plugin")?};
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c=='-' || c=='_') {return Err(err("plugin name must use only letters, digits, - and _"));}
    let lang=match flag(args,"--language") {Some(v)=>v,None=>prompt("Language (lua/rust)", "lua")?};
    if !["rust","lua"].contains(&lang.as_str()) {return Err(err("first release templates support lua or rust"));}
    let runtime=match flag(args,"--runtime") {Some(v)=>v,None=>prompt("Runtime (lua/native/wasm)",if lang=="lua"{"lua"}else{"wasm"})?};
    if (lang=="lua" && runtime!="lua") || (lang=="rust" && runtime!="native" && runtime!="wasm") {
        return Err(err("unsupported language/runtime combination: Lua→lua; Rust→native or wasm"));
    }
    let dir=env::current_dir()?.join(&name);
    if dir.exists(){return Err(err(format!("destination already exists: {}", dir.display())));}
    fs::create_dir(&dir)?; fs::create_dir_all(dir.join("src"))?;
    let id=format!("local.{}",clean_id(&name));
    let manifest=format!("[plugin]\nid = \"{id}\"\nname = \"{name}\"\nversion = \"0.1.0\"\nsdk = \"^1.0\"\nlanguage = \"{lang}\"\nruntime = \"{runtime}\"\nactivation = \"onCommand:{name}\"\ndescription = \"Snack plugin\"\npermissions = [\"all\"]\n\n[targets]\n# Multi-platform paths may be added here in future versions.\n");
    fs::write(dir.join("manifest.toml"),manifest)?;
    if lang=="lua" {
        fs::write(dir.join("src/main.lua"),format!("-- {name}: Snack Lua plugin\n-- Output protocol (0.6): SNACK_MESSAGE and SNACK_INSERT, separated by TAB.\nprint('SNACK_MESSAGE\\tHello from {name}!')\n-- print('SNACK_INSERT\\tHello, Snack!')\n"))?;
    } else {
        let crate_name=clean_id(&name).replace('-',"_");
        fs::write(dir.join("Cargo.toml"),format!("[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n"))?;
        fs::write(dir.join("src/lib.rs"),"// Experimental ABI stub; no Snack SDK host bindings yet.\n#[no_mangle]\npub extern \"C\" fn snack_plugin_init() -> i32 { 0 }\n")?;
    }
    println!("Created {} ({lang}/{runtime})\nNext: snackctl compiler {}",dir.display(),dir.display());
    Ok(())
}
fn copy_tree(src:&Path,dst:&Path)->io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let e=entry?; let file=e.file_name(); let name=file.to_string_lossy();
        if ["target", ".git", "dist", ".snack-backups"].contains(&name.as_ref()){continue;}
        let from=e.path(); let to=dst.join(file);
        let kind=fs::symlink_metadata(&from)?.file_type();
        if kind.is_symlink(){return Err(err("cannot back up symlinks inside plugin project"));}
        if kind.is_dir(){copy_tree(&from,&to)?;} else if kind.is_file(){fs::copy(&from,&to)?;}
    }
    Ok(())
}
fn backup(dir:&Path)->io::Result<PathBuf> {
    let canon=fs::canonicalize(dir)?;
    let parent=canon.parent().ok_or_else(|| err("invalid project path"))?;
    let name=canon.file_name().ok_or_else(|| err("invalid project name"))?.to_string_lossy();
    let backups=parent.join(".snack-backups");fs::create_dir_all(&backups)?;
    let ts=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e|err(e.to_string()))?.as_secs();
    let mut dest=backups.join(format!("{name}-{ts}"));
    let mut n=1;while dest.exists(){dest=backups.join(format!("{name}-{ts}-{n}"));n+=1;}
    copy_tree(&canon,&dest)?;
    println!("Source backup: {}",dest.display());
    Ok(dest)
}
fn run_compiler(dir:&Path,fields:&std::collections::HashMap<String,String>)->io::Result<()> {
    let lang=fields.get("language").map(String::as_str).unwrap_or("");
    let runtime=fields["runtime"].as_str();
    if lang=="lua" && runtime=="lua" {
        if !dir.join("src/main.lua").is_file(){return Err(err("Lua plugin requires src/main.lua"));}
        println!("Lua source prepared (no bytecode compiler needed)");
        return Ok(());
    }
    if lang!="rust" || (runtime!="native" && runtime!="wasm") {
        return Err(err(format!("no compiler adapter yet for {lang}/{runtime}; source backup retained")));
    }
    let cargo=dir.join("Cargo.toml"); if !cargo.is_file(){return Err(err("Rust plugin requires Cargo.toml"));}
    let mut cmd=Command::new("cargo");
    cmd.arg("build").arg("--release").arg("--manifest-path").arg(cargo);
    if runtime=="wasm" {cmd.arg("--target").arg("wasm32-wasip1");}
    println!("Compiling Rust plugin ({runtime})...");
    let status=cmd.status().map_err(|e|err(format!("could not launch cargo: {e}")))?;
    if !status.success(){return Err(err("Cargo compilation failed; backup is safe"));}
    let c=fs::read_to_string(dir.join("Cargo.toml"))?;
    let val:toml::Value=c.parse().map_err(|e|err(format!("invalid Cargo.toml: {e}")))?;
    let crate_name=val.get("package").and_then(|p|p.get("name")).and_then(|v|v.as_str()).ok_or_else(||err("Cargo.toml missing package.name"))?.replace('-',"_");
    let source=if runtime=="wasm" {dir.join(format!("target/wasm32-wasip1/release/{crate_name}.wasm"))}
    else if cfg!(target_os="windows") {dir.join(format!("target/release/{crate_name}.dll"))}
    else if cfg!(target_os="macos") {dir.join(format!("target/release/lib{crate_name}.dylib"))}
    else {dir.join(format!("target/release/lib{crate_name}.so"))};
    if !source.is_file(){return Err(err(format!("compiled artifact not found: {}",source.display())));}
    let runtime_dir=dir.join("runtime");fs::create_dir_all(&runtime_dir)?;
    let target=runtime_dir.join(source.file_name().ok_or_else(||err("bad runtime output"))?);
    fs::copy(&source,&target)?;
    println!("Runtime artifact: {}",target.display());
    Ok(())
}
fn build(dir:&Path,compile:bool)->io::Result<PathBuf> {
    let dir=fs::canonicalize(dir)?;
    let fields=project_manifest(&dir)?;
    if compile {backup(&dir)?;run_compiler(&dir,&fields)?;}
    let dist=dir.join("dist");fs::create_dir_all(&dist)?;
    let output=dist.join(format!("{}-{}.yummy",fields["id"],fields["version"]));
    package::pack(&dir,&output)?;
    let entries=package::read(&output)?;
    let _ = package::manifest_fields(&package::manifest(&entries)?)?;
    println!("Package: {} ({} files, verified)",output.display(),entries.len());
    Ok(output)
}
fn validate(path:&Path)->io::Result<()> {
    if path.is_dir(){let fields=project_manifest(path)?;println!("Valid project: {} v{}",fields["id"],fields["version"]);}
    else {let entries=package::read(path)?;let fields=package::manifest_fields(&package::manifest(&entries)?)?;
        println!("Valid .yummy: {} v{} ({} files, SHA-256 verified; signature not checked)",fields["id"],fields["version"],entries.len());}
    Ok(())
}
fn choose_destination(path:&Path)->io::Result<Option<PathBuf>> {
    let default=env::current_dir()?;
    if cfg!(windows){
        return platform::pick(platform::Picker::Folder,&default,&default);
    }
    let graphical=env::var_os("DISPLAY").is_some() || env::var_os("WAYLAND_DISPLAY").is_some();
    if graphical && Command::new("zenity").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s|s.success()) {
        let out=Command::new("zenity").args(["--file-selection", "--directory", "--title=Snack - Extract .yummy to folder"])
            .arg(format!("--filename={}/",default.display())).output()?;
        if !out.status.success(){return Ok(None);}
        let selection=String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if selection.is_empty(){return Ok(None);}return Ok(Some(PathBuf::from(selection)));
    }
    if !io::stdin().is_terminal(){return Err(err("noninteractive extraction requires --output DIR"));}
    let answer=prompt(&format!("Extract {} to folder",path.display()),&default.display().to_string())?;
    Ok(Some(PathBuf::from(answer)))
}
fn unzip(args:&[String])->io::Result<()> {
    let path=args.iter().find(|x|x.ends_with(".yummy")).map(PathBuf::from).ok_or_else(||err("usage: snackctl unzip -ex file.yummy [--output DIR]"))?;
    let entries=package::read(&path)?;
    let dest=match flag(args,"--output") {Some(x)=>Some(PathBuf::from(x)),None=>choose_destination(&path)?};
    let Some(root)=dest else{println!("Extraction cancelled");return Ok(());};
    let stem=path.file_stem().unwrap_or_default().to_string_lossy();
    let target=root.join(stem.as_ref());
    if target.exists(){return Err(err(format!("destination exists: {} (no overwrite)",target.display())));}
    package::extract(&entries,&target)?;
    println!("Extracted {} files to {}",entries.len(),target.display());Ok(())
}
fn install(path:&Path)->io::Result<()> {
    let entries=package::read(path)?;
    let fields=package::manifest_fields(&package::manifest(&entries)?)?;
    let dest=home_data();fs::create_dir_all(&dest)?;
    let target=dest.join(format!("{}.yummy",fields["id"]));
    if target.exists(){return Err(err("plugin already installed: use uninstall before replacing"));}
    fs::copy(path,&target)?;
    println!("Installed {} (not executed). Run inside Snack via Ctrl+P and confirm once.",fields["id"]);Ok(())
}
fn uninstall(id:&str)->io::Result<()> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)){return Err(err("invalid plugin id"));}
    let path=home_data().join(format!("{id}.yummy"));
    fs::remove_file(&path)?;
    println!("Removed {id}"); Ok(())
}
fn list()->io::Result<()> {
    let dir=home_data();if !dir.exists(){println!("No installed plugins");return Ok(());}
    let mut n=0;
    for item in fs::read_dir(dir)? {
        let path=item?.path();if path.extension().and_then(|e|e.to_str())!=Some("yummy"){continue;}
        n+=1;
        match package::read(&path).and_then(|e|package::manifest_fields(&package::manifest(&e)?)) {
            Ok(fields)=>println!("{} {} [{}]",fields["id"],fields["version"],fields["runtime"]),
            Err(e)=>println!("INVALID {}: {e}",path.display()),
        }
    }
    if n==0{println!("No installed plugins");}Ok(())
}
fn doctor()->io::Result<()> {
    for name in ["cargo","rustc","git","zenity","curl","gh","lua","rust-analyzer","clangd","pylsp"] {
        let found=Command::new(name).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s|s.success());
        println!("{}  {name}",if found{"OK"}else{"--"});
    }
    println!("Plugin directory: {}",home_data().display());
    println!("Official registry: {}", marketplace::OFFICIAL_INDEX);
    if env::var_os("DISPLAY").is_none() && env::var_os("WAYLAND_DISPLAY").is_none(){println!("No graphical display detected; GTK pickers unavailable");}
    Ok(())
}
fn search(args:&[String])->io::Result<()> {
    let q=positional(args).ok_or_else(||err("usage: snackctl search <term>"))?;
    println!("Official Snack registry ({})",marketplace::OFFICIAL_REPO);
    if let Err(e)=marketplace::search(q){eprintln!("  Registry unavailable: {e}");}
    println!("Community GitHub repositories (unverified):");
    let encoded=q.bytes().map(|b|if b.is_ascii_alphanumeric(){(b as char).to_string()}else{format!("%{b:02X}")}).collect::<String>();
    let url=format!("https://api.github.com/search/repositories?q={encoded}+topic%3Asnack-plugin&per_page=10");
    match Command::new("curl").args(["--fail","--location","--silent","--show-error","--proto","=https","--max-time","20","-H","Accept: application/vnd.github+json","-H","User-Agent: snackctl/0.6.1",&url]).output() {
        Ok(output) if output.status.success()=> {
            let json:serde_json::Value=serde_json::from_slice(&output.stdout).map_err(|e|err(e.to_string()))?;
            if let Some(items)=json.get("items").and_then(|x|x.as_array()){
                for item in items{println!("  [GitHub] {} — {}",item["full_name"].as_str().unwrap_or("?"),item["html_url"].as_str().unwrap_or(""));}
                if items.is_empty(){println!("  No community matches");}
            }
        },
        _=>eprintln!("  GitHub community search unavailable"),
    }
    Ok(())
}
fn publish(args:&[String])->io::Result<()> {
    let path=positional(args).ok_or_else(||err("usage: snackctl publish file.yummy [--dry-run] [--github --repo owner/name]"))?;
    let p=Path::new(path);validate(p)?;
    let repo=flag(args,"--repo").unwrap_or_else(||marketplace::OFFICIAL_REPO.to_string());
    if args.iter().any(|x|x=="--marketplace") && repo!=marketplace::OFFICIAL_REPO {
        return Err(err("--marketplace uses the official repo; omit --repo or use criteric/snack-plugins"));
    }
    marketplace::publish(p,&repo,args.iter().any(|x|x=="--dry-run"))
}
fn run() -> io::Result<()> {
    let args:Vec<String>=env::args().skip(1).collect();
    if args.is_empty(){usage();return Ok(());}
    let cmd=args[0].as_str(); let rest=&args[1..];
    if ["--version","-version","-V","version"].contains(&cmd) {println!("snackctl {} (Snack CLI alpha)", env!("CARGO_PKG_VERSION"));return Ok(());}
    if ["--help","help","-h"].contains(&cmd){usage();return Ok(());}
    match cmd {
        "new"=>new(rest),
        "compiler"=>{build(&path_arg(rest),true)?;Ok(())},
        "build"=>{build(&path_arg(rest),false)?;Ok(())},
        "dev"=>{println!("Development compilation (hot reload/debugger not implemented yet)");build(&path_arg(rest),true)?;Ok(())},
        "validate"=>validate(&path_arg(rest)),
        "unzip"=>unzip(rest),
        "install"=>{
            let input=positional(rest).ok_or_else(||err("usage: snackctl install <file.yummy|plugin-id>"))?;
            let path=Path::new(input);
            if path.is_file() {install(path)} else if input.ends_with(".yummy") {Err(err(format!("package not found: {input}")))}
            else {marketplace::install(input,&home_data())}
        },
        "uninstall"=>uninstall(positional(rest).ok_or_else(||err("usage: snackctl uninstall plugin-id"))?),
        "list"=>list(),
        "doctor"=>doctor(),
        "search"=>search(rest),
        "publish"=>publish(rest),
        "update"=>marketplace::notify_updates(&home_data()),
        "settings"=>{
            let binary=env::current_exe()?.with_file_name(if cfg!(windows){"snack.exe"}else{"snack"});
            let program=if binary.is_file(){binary}else{PathBuf::from("snack")};
            let status=Command::new(program).arg("settings").args(rest).status()?;
            if status.success(){Ok(())}else{Err(err("snack settings failed"))}
        },
        _=>{usage();Err(err(format!("unknown command: {cmd}")))},
    }
}
fn main() {
    if let Err(e)=run(){eprintln!("snackctl: {e}");std::process::exit(1);}
}
