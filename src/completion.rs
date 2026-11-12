//! Lightweight built-in completions. This is not an LSP client.
//! Snippets expand only when the user explicitly presses Tab.
use crate::document::Document;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub label: String,
    pub description: String,
    pub replacement: String,
    pub cursor_offset: usize,
    pub replace_chars: usize,
}

fn candidate(label:&str,description:&str,body:&str,replace_chars:usize)->Candidate {
    // A single $0 marks the initial cursor position in our built-in snippets.
    let (replacement,cursor_offset) = if let Some(byte)=body.find("$0") {
        let cursor_offset=body[..byte].chars().count();
        (body.replacen("$0","",1),cursor_offset)
    } else {(body.to_owned(),body.chars().count())};
    Candidate{label:label.into(),description:description.into(),replacement,cursor_offset,replace_chars}
}

pub fn filetype(doc:&Document)->String {
    doc.path.extension().and_then(|e|e.to_str()).unwrap_or("").to_ascii_lowercase()
}
fn html_like(ext:&str)->bool {matches!(ext,"html"|"htm"|"xhtml")}
fn word_char(c:char)->bool {c.is_alphanumeric() || c=='_'}

pub fn typed_prefix(doc:&Document)->String {
    let line=&doc.lines[doc.y];
    let end=doc.x.min(line.len());
    let mut start=end;
    if html_like(&filetype(doc)) {
        // Emmet abbreviations such as ul>li*5 and div#app remain literal until Tab.
        while start>0 && matches!(line[start-1], 'a'..='z'|'A'..='Z'|'0'..='9'|'_'|'-'|'.'|'#'|'>'|'+'|'*'|'!'|':'|'$'|'{'|'}') {start-=1;}
    } else {
        while start>0 && word_char(line[start-1]) {start-=1;}
    }
    line[start..end].iter().collect()
}

fn definitions(ext:&str)->Vec<(&'static str,&'static str)> {
    match ext {
        "html"|"htm"|"xhtml" => vec![
            ("!", "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n    <meta charset=\"UTF-8\">\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n    <title>Document</title>\n</head>\n<body>\n    $0\n</body>\n</html>"),
            ("html:5", "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n    <meta charset=\"UTF-8\">\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n    <title>Document</title>\n</head>\n<body>\n    $0\n</body>\n</html>"),
            ("div","<div>$0</div>"), ("span","<span>$0</span>"),
            ("a","<a href=\"\">$0</a>"), ("img","<img src=\"$0\" alt=\"\">"),
            ("ul","<ul>\n    <li>$0</li>\n</ul>"), ("li","<li>$0</li>"),
            ("button","<button type=\"button\">$0</button>"),
            ("input","<input type=\"text\" name=\"$0\">"),
            ("script","<script>\n    $0\n</script>"),
            ("link","<link rel=\"stylesheet\" href=\"$0\">"),
            ("main","<main>\n    $0\n</main>"), ("section","<section>\n    $0\n</section>"),
            ("header","<header>\n    $0\n</header>"), ("footer","<footer>\n    $0\n</footer>"),
            ("nav","<nav>\n    $0\n</nav>"), ("form","<form action=\"\" method=\"post\">\n    $0\n</form>"),
        ],
        "rs" => vec![
            ("main","fn main() {\n    $0\n}"), ("fn","fn name() {\n    $0\n}"),
            ("println","println!(\"$0\");"), ("eprintln","eprintln!(\"$0\");"),
            ("struct","struct Name {\n    $0\n}"),
            ("impl","impl Name {\n    $0\n}"),
            ("test","#[test]\nfn test_name() {\n    $0\n}"),
            ("if","if condition {\n    $0\n}"),
            ("for","for item in items {\n    $0\n}"),
            ("match","match value {\n    $0\n}"),
            ("use","use $0;"),
        ],
        "py" => vec![
            ("def","def function_name():\n    $0"),
            ("class","class ClassName:\n    def __init__(self):\n        $0"),
            ("ifmain","if __name__ == \"__main__\":\n    $0"),
            ("for","for item in iterable:\n    $0"),
            ("if","if condition:\n    $0"),
            ("try","try:\n    $0\nexcept Exception as exc:\n    print(exc)"),
            ("with","with open(\"file.txt\", encoding=\"utf-8\") as f:\n    $0"),
            ("print","print($0)"),
        ],
        "c"|"h"|"cpp"|"cc"|"cxx"|"hpp" => vec![
            ("main","int main(void) {\n    $0\n    return 0;\n}"),
            ("include","#include <$0>"), ("printf","printf(\"$0\\n\");"),
            ("for","for (int i = 0; i < n; ++i) {\n    $0\n}"),
            ("if","if (condition) {\n    $0\n}"),
            ("struct","struct Name {\n    $0\n};"),
        ],
        "js"|"jsx"|"ts"|"tsx"|"mjs"|"cjs" => vec![
            ("log","console.log($0);"),
            ("function","function name() {\n    $0\n}"),
            ("arrow","const name = () => {\n    $0\n};"),
            ("for","for (const item of items) {\n    $0\n}"),
            ("if","if (condition) {\n    $0\n}"),
            ("async","async function name() {\n    $0\n}"),
            ("import","import { $0 } from \"module\";"),
        ],
        "json"|"jsonc" => vec![
            ("obj","{\n    \"$0\": null\n}"),
            ("arr","[\n    $0\n]"),
            ("true","true"),("false","false"),("null","null"),
        ],
        "sh"|"bash"|"zsh" => vec![
            ("shebang","#!/usr/bin/env bash\nset -euo pipefail\n$0"),
            ("if","if [[ condition ]]; then\n    $0\nfi"),
            ("for","for item in items; do\n    $0\ndone"),
            ("func","function_name() {\n    $0\n}"),
            ("echo","echo \"$0\""),
        ],
        "java" => vec![
            ("class","public class Main {\n    $0\n}"),
            ("main","public static void main(String[] args) {\n    $0\n}"),
            ("sysout","System.out.println($0);"),
            ("for","for (int i = 0; i < n; i++) {\n    $0\n}"),
            ("if","if (condition) {\n    $0\n}"),
        ],
        "go" => vec![
            ("main","package main\n\nimport \"fmt\"\n\nfunc main() {\n    $0\n}"),
            ("func","func name() {\n    $0\n}"),
            ("fmt","fmt.Println($0)"),
            ("for","for i := 0; i < n; i++ {\n    $0\n}"),
            ("if","if condition {\n    $0\n}"),
            ("err","if err != nil {\n    $0\n}"),
        ],
        "css"|"scss" => vec![
            ("flex","display: flex;\njustify-content: $0;\nalign-items: center;"),
            ("grid","display: grid;\ngrid-template-columns: $0;"),
            ("margin","margin: $0;"),("padding","padding: $0;"),
            ("color","color: $0;"),("display","display: $0;"),
        ],
        _ => vec![],
    }
}
fn keywords(ext:&str)->&'static [&'static str] {
    match ext {
        "html"|"htm"|"xhtml"=>&["html","head","body","meta","title","section","article","main","footer","header","nav","form","div","span","button","input","label","table","thead","tbody","tr","td","th","style","script"],
        "rs"=>&["async","await","break","const","continue","crate","dyn","else","enum","extern","false","fn","for","if","impl","in","let","loop","match","mod","move","mut","pub","ref","return","self","Self","static","struct","super","trait","true","type","unsafe","use","where","while","String","Vec","Option","Result","Some","None","Ok","Err","println"],
        "py"=>&["and","as","assert","async","await","break","class","continue","def","del","elif","else","except","False","finally","for","from","global","if","import","in","is","lambda","None","nonlocal","not","or","pass","raise","return","self","True","try","while","with","yield","print","range","len","str","int"],
        "c"|"h"|"cpp"|"cc"|"cxx"|"hpp"=>&["auto","bool","break","case","char","class","const","continue","default","delete","do","double","else","enum","false","float","for","if","include","int","long","namespace","new","nullptr","private","protected","public","return","short","signed","sizeof","static","struct","switch","template","this","true","typedef","union","unsigned","using","virtual","void","volatile","while","printf","scanf"],
        "js"|"jsx"|"ts"|"tsx"|"mjs"|"cjs"=>&["async","await","break","case","catch","class","const","continue","debugger","default","delete","do","else","export","extends","false","finally","for","from","function","if","import","in","instanceof","interface","let","new","null","of","return","static","super","switch","this","throw","true","try","typeof","undefined","var","void","while","yield","console","document","window","setTimeout","Promise"],
        "json"|"jsonc"=>&["true","false","null"],
        "sh"|"bash"|"zsh"=>&["case","do","done","echo","elif","else","esac","eval","exit","export","false","fi","for","function","if","in","local","printf","read","return","set","shift","source","then","true","unset","until","while"],
        "java"=>&["abstract","boolean","break","case","catch","class","const","continue","default","do","double","else","enum","extends","false","final","finally","float","for","if","implements","import","instanceof","int","interface","long","native","new","null","package","private","protected","public","return","short","static","super","switch","synchronized","this","throw","throws","true","try","void","while","String","System"],
        "go"=>&["break","case","chan","const","continue","default","defer","else","fallthrough","false","for","func","go","goto","if","import","interface","map","nil","package","range","return","select","struct","switch","true","type","var","fmt","make","append","len"],
        "css"|"scss"=>&["align-items","background","border","color","display","flex","font-family","font-size","gap","grid","height","justify-content","margin","padding","position","width"],
        _=>&[],
    }
}

pub fn suggestions(doc:&Document, forced:bool)->Vec<Candidate> {
    let ext=filetype(doc);
    let prefix=typed_prefix(doc);
    if prefix.is_empty() && !forced {return vec![];}
    if prefix.chars().count()<2 && !forced && prefix!="!" && !prefix.starts_with('#') && !prefix.starts_with('.') {return vec![];}
    let mut result=vec![];
    let mut seen=BTreeSet::new();
    if html_like(&ext) {
        if let Some(expansion)=crate::emmet::expand(&prefix) {
            let label=if prefix=="!" || prefix=="html:5" {"HTML5 boilerplate"}else{"Emmet HTML element"};
            result.push(candidate(&prefix,label,&expansion,prefix.chars().count()));
            seen.insert(prefix.clone());
        }
    }
    for (trigger,body) in definitions(&ext) {
        if (forced && prefix.is_empty()) || (!prefix.is_empty() && trigger.starts_with(&prefix)) {
            if seen.insert(trigger.to_string()) {
                result.push(candidate(trigger,"Snippet",body,prefix.chars().count()));
            }
        }
    }
    for word in keywords(&ext) {
        if (forced && prefix.is_empty()) || (!prefix.is_empty() && word.starts_with(&prefix) && *word!=prefix.as_str()) {
            if seen.insert((*word).to_string()) {result.push(candidate(word,"Keyword",word,prefix.chars().count()));}
        }
    }
    // File-local names offer useful completion even without a language server.
    for line in &doc.lines {
        let mut word=String::new();
        for c in line.iter().copied().chain(std::iter::once(' ')) {
            if word_char(c) { if word.len()<128 {word.push(c);} }
            else if !word.is_empty() {
                if word!=prefix && (forced && prefix.is_empty() || word.starts_with(&prefix)) && seen.insert(word.clone()) {
                    result.push(candidate(&word,"Document",&word,prefix.chars().count()));
                }
                word.clear();
            }
        }
        if result.len()>300 {break;}
    }
    result.truncate(64);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    fn doc(name:&str,text:&str)->Document {
        let mut d=Document::open(PathBuf::from(name)).unwrap();
        d.lines=text.split('\n').map(|s|s.chars().collect()).collect();
        d.y=d.lines.len()-1;d.x=d.lines[d.y].len();d
    }
    #[test] fn html_exclamation_expands() {
        let d=doc("test.html","!");let s=suggestions(&d,false);
        assert_eq!(s[0].label,"!");assert!(s[0].replacement.contains("<!DOCTYPE html>"));
        assert!(s[0].replacement.contains("<body>"));
        assert!(!s[0].replacement.contains("$0"));
    }
    #[test] fn html_id_abbreviation() {
        let d=doc("test.html","div#app");let s=suggestions(&d,false);
        assert_eq!(s[0].replacement,"<div id=\"app\"></div>");
    }
    #[test] fn rust_snippet() {
        let d=doc("test.rs","main");let s=suggestions(&d,false);
        assert!(s.iter().any(|s|s.replacement.contains("fn main()")));
    }
    #[test] fn document_completion() {
        let d=doc("test.py","temperature\ntem");let s=suggestions(&d,false);
        assert!(s.iter().any(|s|s.label=="temperature"));
    }
}
