use std::sync::LazyLock;

use proc_macro2::{Delimiter as TokenDelimiter, Spacing, TokenStream, TokenTree};
use quote::ToTokens;
use regex::Regex;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;

const ARM9_HEADER: &str = "/* ====== ARM9 ====== */";
const ARM7_HEADER: &str = "/* ====== ARM7 ====== */";
const CFG_CLOSE: &str = "/*cfg_select*/";
pub const POSITION_WARN: &str = "/// WARN: Positioning failed, no surrounding link_sections found to position against";
const ASM_START: &str = "/*(ASM_START";
const ASM_END: &str = "ASM_END)*/";

static CFG_OPEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"cfg_select\s*!\s*\{").unwrap());
static ARM9_OPEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A\s*armv5te\s*=>\s*\{").unwrap());
static MIDDLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A\s*\}\s*armv4t\s*=>\s*\{").unwrap());
static ENDING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A\s*\}\s*_\s*=>\s*\{\s*\}\s*\}\s*\z").unwrap());
static BLOCK_COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)/\*.*?\*/").unwrap());

struct Structure {
    cfg_start: usize,
    arm9: (usize, usize),
    arm7: (usize, usize),
}

enum Parsed {
    NoCfg,
    Cfg(Structure),
    Broken(usize, String),
}

#[derive(PartialEq, Eq)]
enum Delimiter {
    Arm9,
    Arm7,
    CfgClose,
    Other,
}

fn classify(comment: &str) -> Delimiter {
    let inner = comment.trim_start_matches("/*").trim_end_matches("*/");
    let alnum = inner.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_lowercase();
    let only = |allowed: &[char]| inner.chars().filter(|c| !c.is_ascii_alphanumeric()).all(|c| c.is_whitespace() || allowed.contains(&c));
    match alnum.as_str() {
        "arm9" if only(&['*', '=']) => Delimiter::Arm9,
        "arm7" if only(&['*', '=']) => Delimiter::Arm7,
        "cfgselect" if only(&['*', '_']) => Delimiter::CfgClose,
        _ => Delimiter::Other,
    }
}

fn line_of(text: &str, byte: usize) -> usize {
    text[..byte].matches('\n').count()
}

fn next_comment(text: &str, pos: usize) -> Option<(usize, usize)> {
    let start = pos + (text[pos..].len() - text[pos..].trim_start().len());
    let end = start + text[start..].strip_prefix("/*")?.find("*/")? + 4;
    Some((start, end))
}

fn parse_structure(text: &str) -> Parsed {
    let Some(open) = CFG_OPEN.find(text) else { return Parsed::NoCfg };
    let broken = |at: usize, msg: &str| Parsed::Broken(line_of(text, at), msg.to_string());
    let delimiter_at = |pos: usize, want: Delimiter| next_comment(text, pos).filter(|(s, e)| classify(&text[*s..*e]) == want);
    let closer = |from: usize, after: &Regex| {
        BLOCK_COMMENT
            .find_iter(&text[from..])
            .map(|m| (from + m.start(), from + m.end()))
            .find(|(s, e)| classify(&text[*s..*e]) == Delimiter::CfgClose && after.is_match(&text[*e..]))
    };

    let Some(a) = ARM9_OPEN.find(&text[open.end()..]) else {
        return broken(open.end(), "expected `armv5te => {` after `cfg_select! {`");
    };
    let pos = open.end() + a.end();
    let Some((_, arm9_start)) = delimiter_at(pos, Delimiter::Arm9) else {
        return broken(pos, "expected the ARM9 delimiter comment after `armv5te => {`");
    };
    let Some((arm9_end, c1e)) = closer(arm9_start, &MIDDLE) else {
        return broken(arm9_start, "expected a `/*cfg_select*/ }` followed by `armv4t => {` to close the ARM9 section");
    };
    let pos = c1e + MIDDLE.find(&text[c1e..]).unwrap().end();
    let Some((_, arm7_start)) = delimiter_at(pos, Delimiter::Arm7) else {
        return broken(pos, "expected the ARM7 delimiter comment after `armv4t => {`");
    };
    let Some((arm7_end, _)) = closer(arm7_start, &ENDING) else {
        return broken(arm7_start, "expected `/*cfg_select*/ } _ => {} }` closing the ARM7 section at the end of the file");
    };
    Parsed::Cfg(Structure { cfg_start: open.start(), arm9: (arm9_start, arm9_end), arm7: (arm7_start, arm7_end) })
}

fn trim_body(body: &str) -> &str {
    let first = body.find(|c: char| !c.is_whitespace()).unwrap_or(body.len());
    let line_start = body[..first].rfind('\n').map_or(first, |i| i + 1);
    body[line_start..].trim_end()
}

pub fn fix_structure(text: &str) -> Option<String> {
    canonical(text, parse_structure(text))
}

fn canonical(text: &str, parsed: Parsed) -> Option<String> {
    let s = match parsed {
        Parsed::NoCfg => return Some(format!("{}\n\n", text.trim_end())),
        Parsed::Broken(..) => return None,
        Parsed::Cfg(s) => s,
    };
    let body = |r: (usize, usize)| match trim_body(&text[r.0..r.1]) {
        "" => "\n".to_string(),
        b => format!("{b}\n\n"),
    };
    let pre = text[..s.cfg_start].trim_end();
    let pre = if pre.is_empty() { String::new() } else { format!("{pre}\n\n") };
    Some(format!(
        "{pre}cfg_select! {{\narmv5te => {{\n\n{ARM9_HEADER}\n\n{}{CFG_CLOSE} }}\narmv4t => {{\n\n{ARM7_HEADER}\n\n{}{CFG_CLOSE} }} _\n=> {{}} }}\n\n",
        body(s.arm9),
        body(s.arm7)
    ))
}

fn structure_warnings(text: &str) -> Vec<(usize, String)> {
    let parsed = parse_structure(text);
    let has_cfg = match &parsed {
        Parsed::Broken(line, msg) => return vec![(*line, format!("cfg_select layout not recognized: {msg}"))],
        Parsed::Cfg(_) => true,
        Parsed::NoCfg => false,
    };
    match canonical(text, parsed) {
        Some(canon) if canon != text && has_cfg => {
            let diff = text.bytes().zip(canon.bytes()).position(|(a, b)| a != b).unwrap_or(text.len().min(canon.len()));
            vec![(line_of(text, diff), "the cfg_select layout differs from the canonical structure starting here (use `cargo xtask corpus fix` to fix)".into())]
        }
        Some(canon) if canon != text => {
            vec![(line_of(text, text.trim_end().len()), "file should end with exactly two newlines after its last line (`…\\n\\n`, two blank lines in editors)".into())]
        }
        _ => Vec::new(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Shared,
    Arm9,
    Arm7,
    Whole,
    Module,
}

struct Container {
    part: Part,
    label: String,
    entries: Vec<Entry>,
}

struct Entry {
    lead: usize,
    first: usize,
    last: usize,
    section: Option<(String, usize)>,
    label: Option<String>,
}

impl Entry {
    fn prefix(&self) -> Option<&str> {
        self.section.as_ref().and_then(|(s, _)| prefix_of(s))
    }

    fn sec(&self) -> &str {
        self.section.as_ref().map_or("", |(s, _)| s.as_str())
    }

    fn line(&self) -> usize {
        self.section.as_ref().map_or(self.first, |(_, l)| *l)
    }

    fn described(&self) -> String {
        match &self.label {
            Some(l) => format!("\"{}\" ({l})", self.sec()),
            None => format!("\"{}\"", self.sec()),
        }
    }
}

fn prefix_of(section: &str) -> Option<&str> {
    section.find("origaddr.").map(|i| &section[..i + "origaddr.".len()])
}

struct Items(Vec<syn::Item>);

impl Parse for Items {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut items = Vec::new();
        while !input.is_empty() {
            items.push(input.parse()?);
        }
        Ok(Items(items))
    }
}

fn attrs(item: &syn::Item) -> &[syn::Attribute] {
    use syn::Item::*;
    match item {
        Const(i) => &i.attrs,
        Enum(i) => &i.attrs,
        ExternCrate(i) => &i.attrs,
        Fn(i) => &i.attrs,
        ForeignMod(i) => &i.attrs,
        Impl(i) => &i.attrs,
        Macro(i) => &i.attrs,
        Mod(i) => &i.attrs,
        Static(i) => &i.attrs,
        Struct(i) => &i.attrs,
        Trait(i) => &i.attrs,
        TraitAlias(i) => &i.attrs,
        Type(i) => &i.attrs,
        Union(i) => &i.attrs,
        Use(i) => &i.attrs,
        _ => &[],
    }
}

fn label(item: &syn::Item) -> Option<String> {
    use syn::Item::*;
    Some(match item {
        Fn(i) => format!("fn {}", i.sig.ident),
        Static(i) => match i.mutability {
            syn::StaticMutability::Mut(_) => format!("static mut {}", i.ident),
            _ => format!("static {}", i.ident),
        },
        Const(i) => format!("const {}", i.ident),
        Struct(i) => format!("struct {}", i.ident),
        Enum(i) => format!("enum {}", i.ident),
        Union(i) => format!("union {}", i.ident),
        Type(i) => format!("type {}", i.ident),
        Trait(i) => format!("trait {}", i.ident),
        Mod(i) => format!("mod {}", i.ident),
        _ => return None,
    })
}

fn link_section(tokens: TokenStream) -> Option<(String, usize)> {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    for (i, t) in trees.iter().enumerate() {
        match t {
            TokenTree::Ident(id) if id == "link_section" => {
                if let (Some(TokenTree::Punct(eq)), Some(TokenTree::Literal(lit))) = (trees.get(i + 1), trees.get(i + 2))
                    && eq.as_char() == '='
                    && let Ok(s) = syn::parse2::<syn::LitStr>(lit.to_token_stream())
                {
                    return Some((s.value(), id.span().start().line - 1));
                }
            }
            TokenTree::Group(g) => {
                if let Some(found) = link_section(g.stream()) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn is_lead_comment(t: &str) -> bool {
    t.starts_with("//") || (t.starts_with("/*") && t.ends_with("*/") && classify(t) == Delimiter::Other)
}

fn lead_start(lines: &[&str], first: usize) -> usize {
    let mut s = first;
    while s > 0 {
        let t = lines[s - 1].trim();
        if t.ends_with(ASM_END) {
            match (0..s).rev().find(|k| lines[*k].trim_start().starts_with(ASM_START)) {
                Some(k) => s = k,
                None => break,
            }
        } else if !t.is_empty() && is_lead_comment(t) {
            s -= 1;
        } else {
            break;
        }
    }
    s
}

fn collect(items: &[syn::Item], part: Part, label: String, lines: &[&str], out: &mut Vec<Container>) {
    let entries = items
        .iter()
        .map(|item| {
            let span = item.span();
            let first = span.start().line - 1;
            Entry {
                lead: lead_start(lines, first),
                first,
                last: span.end().line - 1,
                section: attrs(item).iter().find_map(|a| link_section(a.meta.to_token_stream())),
                label: self::label(item),
            }
        })
        .collect();
    out.push(Container { part, label, entries });
    for item in items {
        if let syn::Item::Mod(m) = item
            && let Some((_, inner)) = &m.content
        {
            collect(inner, Part::Module, format!("module {}", m.ident), lines, out);
        }
    }
}

fn is_cfg_select(item: &syn::Item) -> bool {
    matches!(item, syn::Item::Macro(m) if m.mac.path.is_ident("cfg_select"))
}

fn cfg_arms(tokens: TokenStream) -> syn::Result<Vec<(String, Vec<syn::Item>)>> {
    let mut arms = Vec::new();
    let mut pattern: Vec<TokenTree> = Vec::new();
    for t in tokens {
        match t {
            TokenTree::Group(g) if g.delimiter() == TokenDelimiter::Brace && ends_with_arrow(&pattern) => {
                pattern.truncate(pattern.len() - 2);
                let name = pattern.drain(..).map(|t| t.to_string()).collect::<String>();
                arms.push((name, syn::parse2::<Items>(g.stream())?.0));
            }
            TokenTree::Punct(p) if p.as_char() == ',' && pattern.is_empty() => {}
            other => pattern.push(other),
        }
    }
    Ok(arms)
}

fn ends_with_arrow(tokens: &[TokenTree]) -> bool {
    matches!(
        tokens,
        [.., TokenTree::Punct(eq), TokenTree::Punct(gt)] if eq.as_char() == '=' && eq.spacing() == Spacing::Joint && gt.as_char() == '>'
    )
}

fn containers(text: &str) -> Result<Vec<Container>, (usize, String)> {
    let err = |e: syn::Error| (e.span().start().line.saturating_sub(1), e.to_string());
    let file = syn::parse_file(text).map_err(err)?;
    let lines: Vec<&str> = text.split('\n').collect();
    let cfg = file.items.iter().find_map(|i| match i {
        syn::Item::Macro(m) if is_cfg_select(i) => Some(m),
        _ => None,
    });
    let mut out = Vec::new();
    match cfg {
        None => collect(&file.items, Part::Whole, "file".into(), &lines, &mut out),
        Some(m) => {
            let shared: Vec<syn::Item> = file.items.iter().filter(|i| !is_cfg_select(i)).cloned().collect();
            collect(&shared, Part::Shared, "shared section".into(), &lines, &mut out);
            for (pattern, items) in cfg_arms(m.mac.tokens.clone()).map_err(err)? {
                match pattern.as_str() {
                    "armv5te" => collect(&items, Part::Arm9, "ARM9 section".into(), &lines, &mut out),
                    "armv4t" => collect(&items, Part::Arm7, "ARM7 section".into(), &lines, &mut out),
                    _ => {}
                }
            }
        }
    }
    Ok(out)
}

fn groups(c: &Container) -> Vec<Vec<&Entry>> {
    let mut out: Vec<Vec<&Entry>> = Vec::new();
    for e in c.entries.iter().filter(|e| e.prefix().is_some()) {
        match out.iter_mut().find(|g| g[0].prefix() == e.prefix()) {
            Some(g) => g.push(e),
            None => out.push(vec![e]),
        }
    }
    out
}

fn longest_increasing(secs: &[&str]) -> Vec<bool> {
    let n = secs.len();
    let mut len = vec![1usize; n];
    let mut prev = vec![None; n];
    for i in 0..n {
        for j in 0..i {
            if secs[j] < secs[i] && len[j] + 1 > len[i] {
                len[i] = len[j] + 1;
                prev[i] = Some(j);
            }
        }
    }
    let mut keep = vec![false; n];
    let mut cur = (0..n).max_by_key(|i| (len[*i], std::cmp::Reverse(*i)));
    while let Some(i) = cur {
        keep[i] = true;
        cur = prev[i];
    }
    keep
}

pub fn lint(text: &str, rust: bool) -> Vec<(usize, String)> {
    let mut out = structure_warnings(text);
    if rust {
        match containers(text) {
            Err((line, msg)) => out.push((line, format!("could not parse as Rust ({msg}); ordering not checked"))),
            Ok(cs) => {
                for c in &cs {
                    for g in groups(c) {
                        let secs: Vec<&str> = g.iter().map(|e| e.sec()).collect();
                        let keep = longest_increasing(&secs);
                        let kept: Vec<&&Entry> = g.iter().zip(&keep).filter(|(_, k)| **k).map(|(e, _)| e).collect();
                        for (e, _) in g.iter().zip(&keep).filter(|(_, k)| !**k) {
                            if let Some(dup) = g.iter().find(|o| o.sec() == e.sec() && o.line() != e.line()) {
                                out.push((e.line(), format!("link_section {} duplicates line {}", e.described(), dup.line() + 1)));
                                continue;
                            }
                            let hint = match kept.iter().filter(|o| o.sec() < e.sec()).max_by_key(|o| o.sec()) {
                                Some(o) => format!("should be placed after {} at line {}", o.described(), o.line() + 1),
                                None => match kept.iter().filter(|o| o.sec() > e.sec()).min_by_key(|o| o.sec()) {
                                    Some(o) => format!("should be placed before {} at line {}", o.described(), o.line() + 1),
                                    None => "is out of order".into(),
                                },
                            };
                            out.push((e.line(), format!("link_section {} is out of order in the {}; it {hint}", e.described(), c.label)));
                        }
                    }
                }
            }
        }
    }
    out.sort_by_key(|(l, _)| *l);
    out
}

fn reorder_once(text: &str) -> Result<Option<String>, String> {
    let cs = containers(text).map_err(|(l, m)| format!("file does not parse (line {}: {m})", l + 1))?;
    for c in &cs {
        for g in groups(c) {
            if g.windows(2).all(|w| w[0].sec() < w[1].sec()) {
                continue;
            }
            let what = format!("{} group in the {}", g[0].prefix().unwrap(), c.label);
            if g.iter().any(|e| g.iter().filter(|o| o.sec() == e.sec()).count() > 1) {
                return Err(format!("{what}: duplicate sections"));
            }
            if c.entries.windows(2).any(|w| w[0].last >= w[1].lead) {
                return Err(format!("{what}: items share lines with their neighbours"));
            }
            let lines: Vec<&str> = text.split('\n').collect();
            let mut sorted: Vec<&&Entry> = g.iter().collect();
            sorted.sort_by_key(|e| e.sec());
            let mut out: Vec<&str> = Vec::new();
            let mut cursor = 0;
            for (slot, item) in g.iter().zip(sorted) {
                out.extend(&lines[cursor..slot.lead]);
                out.extend(&lines[item.lead..=item.last]);
                cursor = slot.last + 1;
            }
            out.extend(&lines[cursor..]);
            return Ok(Some(out.join("\n")));
        }
    }
    Ok(None)
}

pub fn fix_ordering(text: &str) -> (String, Vec<String>) {
    let mut cur = text.to_string();
    for _ in 0..64 {
        match reorder_once(&cur) {
            Ok(Some(next)) => cur = next,
            Ok(None) => return (cur, Vec::new()),
            Err(e) => return (cur, vec![format!("could not reorder: {e}")]),
        }
    }
    (cur, vec!["gave up reordering after 64 passes".into()])
}

pub struct Placement {
    line: usize,
    indent: String,
    before_item: bool,
}

pub fn position(text: &str, section: &str, arm9: bool) -> Option<Placement> {
    let prefix = prefix_of(section)?;
    let cs = containers(text).ok()?;
    let want = if cs.iter().any(|c| c.part == Part::Shared) {
        if arm9 { Part::Arm9 } else { Part::Arm7 }
    } else {
        Part::Whole
    };
    let c = cs.iter().find(|c| c.part == want)?;
    let items: Vec<&Entry> = c.entries.iter().filter(|e| e.prefix() == Some(prefix)).collect();
    let lines: Vec<&str> = text.split('\n').collect();
    let indent = |e: &Entry| lines[e.first].chars().take_while(|c| c.is_whitespace()).collect::<String>();
    let before = |e: &Entry| Placement { line: e.lead, indent: indent(e), before_item: true };
    let prev = items.iter().enumerate().filter(|(_, e)| e.sec() < section).max_by_key(|(_, e)| e.sec());
    let next = items.iter().enumerate().filter(|(_, e)| e.sec() > section).min_by_key(|(_, e)| e.sec());
    match (prev, next) {
        (Some((p, _)), Some((n, e))) if n == p + 1 => Some(before(e)),
        (None, Some((0, e))) => Some(before(e)),
        (Some((p, e)), None) if p + 1 == items.len() => Some(Placement { line: e.last + 1, indent: indent(e), before_item: false }),
        _ => None,
    }
}

pub fn insert_at(text: &str, placement: &Placement, body: &[String]) -> String {
    let mut lines: Vec<String> = text.split('\n').map(String::from).collect();
    let mut block: Vec<String> = body.iter().map(|l| if l.is_empty() { String::new() } else { format!("{}{l}", placement.indent) }).collect();
    if placement.before_item {
        block.push(String::new());
    } else {
        block.insert(0, String::new());
    }
    let at = placement.line.min(lines.len());
    lines.splice(at..at, block);
    lines.join("\n")
}
