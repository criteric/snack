//! A small, dependency-free Emmet subset for Snack.
//! Supported: tag, #id, .class, > child, + sibling, *N repetition, {text}, !.
//! Expansion is explicit (Tab); # stays literal until expansion.
#[derive(Debug, Clone)]
struct Node { tag: String, id: Option<String>, classes: Vec<String>, body: String, count: usize, children: Vec<Node> }

const BOILERPLATE: &str = "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n    <meta charset=\"UTF-8\">\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n    <title>Document</title>\n</head>\n<body>\n    $0\n</body>\n</html>";

fn ident(bytes: &[u8], i: &mut usize) -> String {
    let start = *i;
    while *i < bytes.len() && (bytes[*i].is_ascii_alphanumeric() || matches!(bytes[*i], b'-'|b'_'|b'$')) { *i += 1; }
    String::from_utf8_lossy(&bytes[start..*i]).to_string()
}
fn atom(s: &str) -> Option<Node> {
    let bytes = s.as_bytes(); let mut i = 0;
    let tag = ident(bytes, &mut i);
    let tag = if tag.is_empty() { "div".to_owned() } else { tag };
    let mut node = Node {tag, id:None, classes:vec![], body:String::new(), count:1, children:vec![]};
    let mut specified = i > 0;
    while i < bytes.len() {
        match bytes[i] {
            b'#' => { i+=1;let s=ident(bytes,&mut i); if s.is_empty(){return None;}node.id=Some(s);specified=true; },
            b'.' => { i+=1;let s=ident(bytes,&mut i); if s.is_empty(){return None;}node.classes.push(s);specified=true; },
            b'*' => {i+=1; let start=i;while i<bytes.len()&&bytes[i].is_ascii_digit(){i+=1;}
                let n:usize=s.get(start..i)?.parse().ok()?;if !(1..=32).contains(&n){return None;}node.count=n;specified=true;},
            b'{' => {i+=1;let start=i;while i<bytes.len()&&bytes[i]!=b'}'{i+=1;}
                if i>=bytes.len(){return None;}node.body=s.get(start..i)?.to_owned();i+=1;specified=true;},
            _ => return None,
        }
    }
    if !specified {return None;}Some(node)
}
fn tokenize(input:&str)->Option<(Vec<String>,Vec<char>)> {
    if input.len()>256 || input.is_empty() || !input.is_ascii() {return None;}
    let mut tokens=Vec::new();let mut ops=Vec::new();let mut start=0;let mut brace=false;
    for (i,c) in input.char_indices() {
        match c {'{'=>brace=true,'}'=>brace=false,'>'|'+' if !brace=>{
            if i==start{return None;}tokens.push(input[start..i].to_owned());ops.push(c);start=i+1;
        },_=>{}}
    }
    if brace || start>=input.len(){return None;}tokens.push(input[start..].to_owned());
    if tokens.len()>48{return None;}Some((tokens,ops))
}
fn parse_expression(tokens:&[String],ops:&[char],pos:&mut usize)->Option<Vec<Node>>{
    let mut result=Vec::new();
    while *pos<tokens.len(){
        let mut item=atom(&tokens[*pos])?;*pos+=1;
        if *pos<=ops.len() && ops[*pos-1]=='>' {
            item.children=parse_expression(tokens,ops,pos)?;
        }
        result.push(item);
        if *pos>ops.len() || ops[*pos-1]!='+' {break;}
    }
    Some(result)
}
fn escape_attr(s:&str)->String{s.replace('&',"&amp;").replace('"',"&quot;").replace('<',"&lt;")}
fn render(nodes:&[Node], depth:usize, out:&mut String, marked:&mut bool, total:&mut usize)->Option<()> {
    for node in nodes {
        for n in 0..node.count {
            *total+=1;if *total>256 || out.len()>20_000{return None;}
            if !out.is_empty(){out.push('\n');}
            out.push_str(&"    ".repeat(depth));out.push('<');out.push_str(&node.tag);
            if let Some(id)=&node.id {out.push_str(" id=\"");out.push_str(&escape_attr(&id.replace('$',&(n+1).to_string())));out.push('"');}
            if !node.classes.is_empty(){out.push_str(" class=\"");out.push_str(&escape_attr(&node.classes.join(" ").replace('$',&(n+1).to_string())));out.push('"');}
            out.push('>');
            if node.children.is_empty(){
                out.push_str(&node.body.replace('$',&(n+1).to_string()));
                if !*marked{out.push_str("$0");*marked=true;}
                out.push_str("</");out.push_str(&node.tag);out.push('>');
            } else {
                let mut nested=String::new();render(&node.children,depth+1,&mut nested,marked,total)?;
                out.push('\n');out.push_str(&nested);out.push('\n');out.push_str(&"    ".repeat(depth));out.push_str("</");out.push_str(&node.tag);out.push('>');
            }
        }
    }
    Some(())
}
pub fn expand(input:&str)->Option<String>{
    if input=="!" || input=="html:5"{return Some(BOILERPLATE.to_owned());}
    let (tokens,ops)=tokenize(input)?;let mut pos=0;let tree=parse_expression(&tokens,&ops,&mut pos)?;
    if pos!=tokens.len(){return None;}
    let mut result=String::new();let mut marked=false;let mut total=0;
    render(&tree,0,&mut result,&mut marked,&mut total)?;Some(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn html5(){assert!(expand("!").unwrap().contains("<!DOCTYPE html>"));}
    #[test] fn id(){assert_eq!(expand("div#app").unwrap().replace("$0",""),"<div id=\"app\"></div>");}
    #[test] fn nested_repeat(){let s=expand("ul>li.item*5").unwrap();assert_eq!(s.matches("<li").count(),5);assert!(s.contains("</ul>"));}
    #[test] fn siblings(){let s=expand("section>h1+p").unwrap();assert!(s.contains("<h1>"));assert!(s.contains("<p>"));}
    #[test] fn invalid(){assert!(expand("ul>li*999").is_none());assert!(expand("div>..").is_none());}
}
