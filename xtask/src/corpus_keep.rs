use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::ToTokens;
use syn::spanned::Spanned;

use crate::corpus::Cpu;

pub const DIRECTIVE: &str = "/*(keep_symbols_list)*/";
const SECTION_START: &str = "//list_section_start";
const SECTION_END: &str = "//list_section_end";
const FEATURE: &str = "used_with_arg";

fn list_section(cpu: Cpu) -> &'static str {
    match cpu {
        Cpu::Arm7 => ".rodata.main.keep_symbols_list",
        Cpu::Arm9 => ".rodata.keep_symbols_list",
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Entry {
    cfg: Vec<String>,
    expr: String,
}

pub struct Resolved {
    pub entries: Vec<Entry>,
    pub warning: Option<String>,
}

#[derive(Clone)]
struct Krate {
    name: String,
    extern_name: String,
    root: PathBuf,
}

#[derive(Clone)]
pub struct Binary {
    pub cpu: Cpu,
    package: String,
    crates: Vec<Krate>,
}

impl Binary {
    pub fn load(rsdec: &Path, cpu: Cpu) -> Result<Binary> {
        let out = Command::new("cargo")
            .current_dir(rsdec.join(cpu.dir()))
            .args(["metadata", "--format-version", "1"])
            .output()
            .context("running cargo metadata")?;
        ensure!(out.status.success(), "cargo metadata failed in {}: {}", cpu.dir(), String::from_utf8_lossy(&out.stderr));
        let meta: serde_json::Value = serde_json::from_slice(&out.stdout)?;
        let packages = meta["packages"].as_array().context("cargo metadata: no packages")?;
        let by_id = |id: &str| packages.iter().find(|p| p["id"] == id);
        let target_of = |p: &serde_json::Value, kind: &str| {
            p["targets"].as_array()?.iter().find(|t| t["kind"].as_array().is_some_and(|k| k.iter().any(|x| x == kind))).cloned()
        };
        let members = meta["workspace_members"].as_array().context("cargo metadata: no workspace members")?;
        let (bin_pkg, bin_target) = members
            .iter()
            .filter_map(|id| by_id(id.as_str()?))
            .find_map(|p| Some((p, target_of(p, "bin")?)))
            .with_context(|| format!("no binary crate in the {} workspace", cpu.dir()))?;
        let mut crates = vec![Krate {
            name: bin_target["name"].as_str().unwrap_or_default().replace('-', "_"),
            extern_name: "crate".into(),
            root: PathBuf::from(bin_target["src_path"].as_str().context("bin target has no src_path")?),
        }];
        let node = meta["resolve"]["nodes"]
            .as_array()
            .and_then(|nodes| nodes.iter().find(|n| n["id"] == bin_pkg["id"]))
            .context("cargo metadata: binary crate missing from the resolve graph")?;
        for dep in node["deps"].as_array().into_iter().flatten() {
            let (Some(extern_name), Some(pkg)) = (dep["name"].as_str(), dep["pkg"].as_str().and_then(by_id)) else { continue };
            let Some(lib) = target_of(pkg, "lib").filter(|_| !pkg["source"].is_string()) else { continue };
            crates.push(Krate {
                name: lib["name"].as_str().unwrap_or_default().to_string(),
                extern_name: extern_name.to_string(),
                root: PathBuf::from(lib["src_path"].as_str().unwrap_or_default()),
            });
        }
        Ok(Binary { cpu, package: bin_pkg["name"].as_str().unwrap_or_default().to_string(), crates })
    }

    fn root(&self) -> &Path {
        &self.crates[0].root
    }

    pub fn keep_list_file(&self, texts: &[(PathBuf, String)]) -> Result<Option<PathBuf>> {
        let crate_dir = self.root().parent().context("binary crate root has no directory")?;
        let mut found = Vec::new();
        for (path, text) in texts.iter().filter(|(p, _)| p.starts_with(crate_dir)) {
            if directive_line(text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?.is_some() {
                found.push(path.clone());
            }
        }
        match found.len() {
            0 | 1 => Ok(found.pop()),
            _ => bail!("{} has `{DIRECTIVE}` in several files: {}", self.package, found.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")),
        }
    }

    pub fn require_keep_list(&self, texts: &[(PathBuf, String)]) -> Result<PathBuf> {
        let root_text = match texts.iter().find(|(p, _)| p == self.root()) {
            Some((_, t)) => t.clone(),
            None => fs::read_to_string(self.root())?,
        };
        ensure!(root_text.contains(FEATURE), "the keep list uses #[used(linker)]; add `#![feature({FEATURE})]` to {}", self.root().display());
        self.keep_list_file(texts)?.with_context(|| {
            format!("{} needs a keep list: put `{DIRECTIVE}` on its own line at item level (e.g. in {})", self.package, self.root().display())
        })
    }

    pub fn index(&self, texts: &HashMap<PathBuf, String>) -> Result<Index> {
        let mut items = Vec::new();
        let mut reexports = Vec::new();
        for (k, krate) in self.crates.iter().enumerate() {
            let mut files = Vec::new();
            collect_rs(krate.root.parent().context("crate root has no directory")?, &mut files)?;
            let mut parsed = Vec::new();
            for file in files {
                let Some(module) = module_path(&krate.root, &file) else { continue };
                let text = match texts.get(&file) {
                    Some(t) => t.clone(),
                    None => fs::read_to_string(&file)?,
                };
                if let Ok(ast) = syn::parse_file(&text) {
                    parsed.push((file, module, ast));
                }
            }
            let mut decls: HashMap<Vec<String>, ModDecl> = HashMap::new();
            for (_, module, ast) in &parsed {
                walk_decls(&ast.items, &Scope { module: module.clone(), ..Scope::default() }, &mut decls);
            }
            let is_bin = k == 0;
            for (file, module, ast) in &parsed {
                let chain = |n: usize| decls.get(&module[..n]);
                let checked_from = if is_bin { 2 } else { 1 };
                let scope = Scope {
                    module: module.clone(),
                    only: None,
                    cfg: (1..=module.len()).flat_map(|n| chain(n).map(|d| d.cfg.clone()).unwrap_or_default()).collect(),
                    reachable: (checked_from..=module.len()).all(|n| chain(n).is_some_and(|d| d.public)),
                };
                walk_items(&ast.items, &scope, &FileCtx { krate: k, file, is_bin }, &mut items, &mut reexports);
            }
        }
        apply_reexports(&mut items, &reexports);
        Ok(Index { bin: self.clone(), items })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Fn,
    Static,
}

struct KeepItem {
    krate: usize,
    path: Vec<String>,
    kind: Kind,
    exported: Option<String>,
    only: Option<Cpu>,
    cfg: Vec<String>,
    nameable: bool,
    at: String,
}

pub struct Index {
    bin: Binary,
    items: Vec<KeepItem>,
}

enum Demangled {
    Path(Option<usize>, Vec<String>),
    Qualified(String),
}

impl Index {
    pub fn resolves(&self, symbol: &str) -> bool {
        self.resolve(symbol).is_ok()
    }

    pub fn resolve(&self, symbol: &str) -> Result<Resolved, String> {
        let visible = |i: &&KeepItem| i.only.is_none_or(|c| c == self.bin.cpu);
        let mut hits: Vec<&KeepItem> = self.items.iter().filter(visible).filter(|i| i.exported.as_deref() == Some(symbol)).collect();
        if hits.is_empty() {
            match self.demangle(symbol)? {
                Some(Demangled::Path(krate, segs)) => {
                    hits = self.items.iter().filter(visible).filter(|i| Some(i.krate) == krate && i.path == segs).collect();
                }
                Some(Demangled::Qualified(path)) => {
                    return Ok(Resolved {
                        entries: vec![Entry { cfg: Vec::new(), expr: format!("{path} as *const ()") }],
                        warning: Some(format!(
                            "'{symbol}' kept as `{path}` from its demangled trait path; the tool cannot see trait impls, so \
                             whether that path compiles is left to rustc"
                        )),
                    });
                }
                None => {}
            }
        }
        self.entries_for(symbol, hits)
    }

    fn demangle(&self, symbol: &str) -> Result<Option<Demangled>, String> {
        let Ok(d) = rustc_demangle::try_demangle(symbol) else { return Ok(None) };
        let demangled = format!("{d:#}");
        if demangled.contains('{') || demangled.contains("::<") {
            return Err(format!(
                "'{symbol}' demangles to `{demangled}`, which cannot be named from Rust source (closure, shim or generic instantiation)"
            ));
        }
        let path = match demangled.strip_prefix('<').and_then(|r| r.split_once(">::")) {
            None => demangled.clone(),
            Some((ty, method)) if !ty.contains(" as ") && !ty.contains('<') => format!("{ty}::{method}"),
            Some(_) => {
                let rewritten = rewrite_crates(&demangled, &self.bin.crates)
                    .ok_or_else(|| format!("'{symbol}' demangles to `{demangled}`, which could not be turned into a path"))?;
                return Ok(Some(Demangled::Qualified(rewritten)));
            }
        };
        let mut segs: Vec<String> = path.split("::").map(String::from).collect();
        let krate = self.bin.crates.iter().position(|c| c.name == segs[0]);
        segs.remove(0);
        Ok(Some(Demangled::Path(krate, segs)))
    }

    fn entries_for(&self, symbol: &str, hits: Vec<&KeepItem>) -> Result<Resolved, String> {
        let bin = &self.bin;
        if hits.is_empty() {
            return Err(format!(
                "no item exporting '{symbol}' found in {} or its direct dependencies (items made by macros, #[path] modules and \
                 crates that are not direct dependencies are not visible)",
                bin.package
            ));
        }
        if hits.iter().map(|i| &i.cfg).collect::<BTreeSet<_>>().len() != hits.len() {
            return Err(format!(
                "'{symbol}' matches several items under the same cfg: {}",
                hits.iter().map(|i| i.at.as_str()).collect::<Vec<_>>().join(", ")
            ));
        }
        let mut entries = Vec::new();
        for item in hits {
            if !item.nameable {
                return Err(format!(
                    "'{symbol}' ({}) is not nameable from {}; it, its type and its modules must be pub (or re-exported with pub use)",
                    item.at, bin.package
                ));
            }
            let path = format!("{}::{}", bin.crates[item.krate].extern_name, item.path.join("::"));
            let expr = match item.kind {
                Kind::Fn => format!("{path} as *const ()"),
                Kind::Static => format!("&raw const {path} as *const ()"),
            };
            entries.push(Entry { cfg: item.cfg.clone(), expr });
        }
        Ok(Resolved { entries, warning: None })
    }
}

fn rewrite_crates(demangled: &str, crates: &[Krate]) -> Option<String> {
    let mut expr: syn::ExprPath = syn::parse_str(demangled).ok()?;
    let fix = |path: &mut syn::Path| {
        if let Some(first) = path.segments.first_mut()
            && let Some(k) = crates.iter().find(|k| first.ident == k.name)
        {
            first.ident = syn::Ident::new(&k.extern_name, first.ident.span());
        }
    };
    if let Some(q) = &mut expr.qself
        && let syn::Type::Path(tp) = q.ty.as_mut()
    {
        fix(&mut tp.path);
    }
    fix(&mut expr.path);
    Some(expr.to_token_stream().to_string().replace(" :: ", "::").replace("< ", "<").replace(" >", ">"))
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_rs(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn module_path(root: &Path, file: &Path) -> Option<Vec<String>> {
    if file == root {
        return Some(Vec::new());
    }
    let mut segs: Vec<String> = file.strip_prefix(root.parent()?).ok()?.iter().map(|s| s.to_string_lossy().into_owned()).collect();
    match segs.pop()?.strip_suffix(".rs")? {
        "mod" => {}
        "lib" | "main" if segs.is_empty() => return None,
        stem => segs.push(stem.to_string()),
    }
    Some(segs)
}

#[derive(Clone, Default)]
struct Scope {
    module: Vec<String>,
    only: Option<Cpu>,
    cfg: Vec<String>,
    reachable: bool,
}

impl Scope {
    fn constrain(&mut self, predicate: &str) {
        match Cpu::ALL.into_iter().find(|c| predicate == c.cfg()) {
            Some(c) => self.only = Some(c),
            None if predicate == "_" => {}
            None => self.cfg.push(format!("#[cfg({predicate})]")),
        }
    }

    fn with_attrs(&self, attrs: &[syn::Attribute]) -> Scope {
        let mut s = self.clone();
        for list in attrs.iter().filter(|a| a.path().is_ident("cfg")).filter_map(|a| a.meta.require_list().ok()) {
            s.constrain(&list.tokens.to_string());
        }
        s
    }

    fn with_arm(&self, pattern: &TokenStream) -> Scope {
        let mut s = self.clone();
        s.constrain(&pattern.to_string());
        s
    }

    fn child(&self, name: &syn::Ident) -> Vec<String> {
        let mut p = self.module.clone();
        p.push(name.to_string());
        p
    }
}

struct FileCtx<'a> {
    krate: usize,
    file: &'a Path,
    is_bin: bool,
}

struct ModDecl {
    public: bool,
    cfg: Vec<String>,
}

struct Reexport {
    krate: usize,
    module: Vec<String>,
    name: Option<String>,
    target: Vec<String>,
    public: bool,
}

fn cfg_select_arms(item: &syn::Item) -> Option<Vec<(TokenStream, Vec<syn::Item>)>> {
    let syn::Item::Macro(m) = item else { return None };
    if !m.mac.path.is_ident("cfg_select") {
        return None;
    }
    struct Items(Vec<syn::Item>);
    impl syn::parse::Parse for Items {
        fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
            let mut v = Vec::new();
            while !input.is_empty() {
                v.push(input.parse()?);
            }
            Ok(Items(v))
        }
    }
    let mut out = Vec::new();
    let mut pattern: Vec<TokenTree> = Vec::new();
    for t in m.mac.tokens.clone() {
        match t {
            TokenTree::Group(g) if g.delimiter() == Delimiter::Brace && ends_with_arrow(&pattern) => {
                pattern.truncate(pattern.len() - 2);
                let pat: TokenStream = pattern.drain(..).collect();
                if let Ok(items) = syn::parse2::<Items>(g.stream()) {
                    out.push((pat, items.0));
                }
            }
            TokenTree::Punct(p) if p.as_char() == ',' && pattern.is_empty() => {}
            other => pattern.push(other),
        }
    }
    Some(out)
}

fn ends_with_arrow(tokens: &[TokenTree]) -> bool {
    matches!(
        tokens,
        [.., TokenTree::Punct(eq), TokenTree::Punct(gt)] if eq.as_char() == '=' && eq.spacing() == Spacing::Joint && gt.as_char() == '>'
    )
}

fn attrs_of(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Fn(i) => &i.attrs,
        syn::Item::Static(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

fn exported_name(attrs: &[syn::Attribute], ident: &syn::Ident) -> Option<String> {
    fn scan(tokens: TokenStream, ident: &syn::Ident) -> Option<String> {
        let trees: Vec<TokenTree> = tokens.into_iter().collect();
        trees.iter().enumerate().find_map(|(i, t)| match t {
            TokenTree::Ident(id) if id == "no_mangle" => Some(ident.to_string()),
            TokenTree::Ident(id) if id == "export_name" => match trees.get(i + 2) {
                Some(TokenTree::Literal(lit)) => syn::parse2::<syn::LitStr>(lit.to_token_stream()).ok().map(|s| s.value()),
                _ => None,
            },
            TokenTree::Group(g) => scan(g.stream(), ident),
            _ => None,
        })
    }
    attrs.iter().find_map(|a| scan(a.meta.to_token_stream(), ident))
}

fn is_public(vis: &syn::Visibility, is_bin: bool) -> bool {
    match vis {
        syn::Visibility::Public(_) => true,
        syn::Visibility::Restricted(_) => is_bin,
        syn::Visibility::Inherited => false,
    }
}

fn walk_decls(items: &[syn::Item], scope: &Scope, out: &mut HashMap<Vec<String>, ModDecl>) {
    for item in items {
        if let syn::Item::Mod(m) = item {
            let inner = scope.with_attrs(&m.attrs);
            let path = scope.child(&m.ident);
            let public = !matches!(m.vis, syn::Visibility::Inherited);
            let own_cfg = inner.cfg[scope.cfg.len()..].to_vec();
            out.entry(path.clone()).and_modify(|d| d.public |= public).or_insert(ModDecl { public, cfg: own_cfg });
            if let Some((_, content)) = &m.content {
                walk_decls(content, &Scope { module: path, ..inner }, out);
            }
        } else if let Some(arms) = cfg_select_arms(item) {
            for (pat, inner) in arms {
                walk_decls(&inner, &scope.with_arm(&pat), out);
            }
        }
    }
}

fn type_visibility<'a>(items: &'a [syn::Item], name: &syn::Ident) -> Option<&'a syn::Visibility> {
    items.iter().find_map(|i| match i {
        syn::Item::Struct(s) if s.ident == *name => Some(&s.vis),
        syn::Item::Enum(e) if e.ident == *name => Some(&e.vis),
        syn::Item::Union(u) if u.ident == *name => Some(&u.vis),
        _ => None,
    })
}

fn walk_items(items: &[syn::Item], scope: &Scope, ctx: &FileCtx, out: &mut Vec<KeepItem>, reexports: &mut Vec<Reexport>) {
    let at_root = scope.module.is_empty() && ctx.is_bin;
    for item in items {
        let s = scope.with_attrs(attrs_of(item));
        let mut push = |path: Vec<String>, kind: Kind, exported: Option<String>, public: bool, span: proc_macro2::Span| {
            out.push(KeepItem {
                krate: ctx.krate,
                path,
                kind,
                exported,
                only: s.only,
                cfg: s.cfg.clone(),
                nameable: at_root || (s.reachable && public),
                at: format!("{}:{}", ctx.file.display(), span.start().line),
            });
        };
        match item {
            syn::Item::Fn(f) if f.sig.generics.params.is_empty() => {
                push(s.child(&f.sig.ident), Kind::Fn, exported_name(&f.attrs, &f.sig.ident), is_public(&f.vis, ctx.is_bin), f.span())
            }
            syn::Item::Static(st) => {
                push(s.child(&st.ident), Kind::Static, exported_name(&st.attrs, &st.ident), is_public(&st.vis, ctx.is_bin), st.span())
            }
            syn::Item::Impl(im) if im.trait_.is_none() && im.generics.params.is_empty() => {
                let syn::Type::Path(tp) = im.self_ty.as_ref() else { continue };
                let Some(ty) = tp.path.get_ident() else { continue };
                let ty_public = type_visibility(items, ty).is_none_or(|v| is_public(v, ctx.is_bin));
                for ii in &im.items {
                    let syn::ImplItem::Fn(f) = ii else { continue };
                    if f.sig.generics.params.is_empty() {
                        let mut path = s.child(ty);
                        path.push(f.sig.ident.to_string());
                        let public = ty_public && is_public(&f.vis, ctx.is_bin);
                        push(path, Kind::Fn, exported_name(&f.attrs, &f.sig.ident), public, f.span());
                    }
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, content)) = &m.content {
                    let inner = Scope { module: s.child(&m.ident), reachable: s.reachable && is_public(&m.vis, ctx.is_bin), ..s.clone() };
                    walk_items(content, &inner, ctx, out, reexports);
                }
            }
            syn::Item::Use(u) if !matches!(u.vis, syn::Visibility::Inherited) => {
                let public = s.reachable && is_public(&u.vis, ctx.is_bin);
                collect_use(&u.tree, &s.module, Vec::new(), ctx.krate, public, reexports);
            }
            _ => {
                for (pat, inner) in cfg_select_arms(item).unwrap_or_default() {
                    walk_items(&inner, &s.with_arm(&pat), ctx, out, reexports);
                }
            }
        }
    }
}

fn collect_use(tree: &syn::UseTree, module: &[String], mut prefix: Vec<String>, krate: usize, public: bool, out: &mut Vec<Reexport>) {
    let absolute = |segs: Vec<String>| -> Option<Vec<String>> {
        let mut base: Vec<String> = module.to_vec();
        let mut rest = segs.into_iter().peekable();
        match rest.peek().map(String::as_str) {
            Some("crate") => {
                base.clear();
                rest.next();
            }
            Some("self") => {
                rest.next();
            }
            _ => {}
        }
        while rest.peek().map(String::as_str) == Some("super") {
            base.pop()?;
            rest.next();
        }
        base.extend(rest);
        Some(base)
    };
    let mut add = |segs: Vec<String>, name: Option<String>| {
        if let Some(target) = absolute(segs) {
            out.push(Reexport { krate, module: module.to_vec(), name, target, public });
        }
    };
    match tree {
        syn::UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            collect_use(&p.tree, module, prefix, krate, public, out);
        }
        syn::UseTree::Name(n) => {
            prefix.push(n.ident.to_string());
            add(prefix, Some(n.ident.to_string()));
        }
        syn::UseTree::Rename(r) => {
            prefix.push(r.ident.to_string());
            add(prefix, Some(r.rename.to_string()));
        }
        syn::UseTree::Glob(_) => add(prefix, None),
        syn::UseTree::Group(g) => {
            for t in &g.items {
                collect_use(t, module, prefix.clone(), krate, public, out);
            }
        }
    }
}

fn apply_reexports(items: &mut [KeepItem], reexports: &[Reexport]) {
    for item in items.iter_mut().filter(|i| !i.nameable) {
        let (name, module) = item.path.split_last().expect("item path has a name");
        let via = reexports.iter().filter(|r| r.krate == item.krate && r.public).find_map(|r| match &r.name {
            Some(alias) if r.target == item.path => Some([r.module.clone(), vec![alias.clone()]].concat()),
            None if r.target == module => Some([r.module.clone(), vec![name.clone()]].concat()),
            _ => None,
        });
        if let Some(path) = via {
            item.path = path;
            item.nameable = true;
        }
    }
}

fn comment_depth_per_line(text: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    for line in text.split('\n') {
        out.push(depth);
        let b = line.as_bytes();
        let mut i = 0;
        while i + 1 < b.len() {
            match (b[i], b[i + 1]) {
                (b'/', b'*') => {
                    depth += 1;
                    i += 2;
                }
                (b'*', b'/') if depth > 0 => {
                    depth -= 1;
                    i += 2;
                }
                (b'/', b'/') if depth == 0 => break,
                _ => i += 1,
            }
        }
    }
    out
}

fn directive_line(text: &str) -> Result<Option<usize>, String> {
    let depth = comment_depth_per_line(text);
    let found: Vec<usize> = text.split('\n').enumerate().filter(|(i, l)| l.trim() == DIRECTIVE && depth[*i] == 0).map(|(i, _)| i).collect();
    match found.as_slice() {
        [] => Ok(None),
        [line] => Ok(Some(*line)),
        many => Err(format!("`{DIRECTIVE}` appears more than once, at lines {}", many.iter().map(|l| (l + 1).to_string()).collect::<Vec<_>>().join(", "))),
    }
}

fn section_range(lines: &[&str], directive: usize) -> Option<std::ops::RangeInclusive<usize>> {
    let first = directive + 1;
    if !lines.get(first)?.trim_end().ends_with(SECTION_START) {
        return None;
    }
    let end = (first..lines.len()).find(|&i| lines[i].trim_end().ends_with(SECTION_END))?;
    Some(first..=end)
}

pub fn read(text: &str) -> BTreeSet<Entry> {
    let lines: Vec<&str> = text.split('\n').collect();
    let Some(range) = directive_line(text).ok().flatten().and_then(|d| section_range(&lines, d)) else { return BTreeSet::new() };
    let mut out = BTreeSet::new();
    let mut pending: Vec<String> = Vec::new();
    let mut current: Option<Vec<String>> = None;
    for t in lines[range].iter().map(|l| l.trim()) {
        if t.starts_with("#[cfg") {
            pending.push(t.to_string());
        } else if t.starts_with("static KEEP_SYMBOLS_LIST") {
            current = Some(std::mem::take(&mut pending));
        } else if t.starts_with("]);") {
            current = None;
        } else if let Some(cfg) = &current
            && let Some(expr) = t.strip_suffix(',')
        {
            out.insert(Entry { cfg: cfg.clone(), expr: expr.to_string() });
        } else if !t.starts_with("#[") {
            pending.clear();
        }
    }
    out
}

pub fn render(text: &str, cpu: Cpu, entries: Option<&BTreeSet<Entry>>) -> String {
    let Some(directive) = directive_line(text).ok().flatten() else { return text.to_string() };
    let mut lines: Vec<String> = text.split('\n').map(String::from).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let range = section_range(&refs, directive).unwrap_or(directive + 1..=directive);
    let block = entries.map(|e| section_lines(&lines[directive], cpu, e)).unwrap_or_default();
    lines.splice(range, block);
    lines.join("\n")
}

fn section_lines(directive_line: &str, cpu: Cpu, entries: &BTreeSet<Entry>) -> Vec<String> {
    let i: String = directive_line.chars().take_while(|c| c.is_whitespace()).collect();
    let mut groups: Vec<(&[String], Vec<&str>)> = vec![(&[], Vec::new())];
    for e in entries {
        match groups.iter_mut().find(|(c, _)| *c == e.cfg.as_slice()) {
            Some((_, v)) => v.push(&e.expr),
            None => groups.push((&e.cfg, vec![&e.expr])),
        }
    }
    if groups.len() > 1 && groups[0].1.is_empty() {
        groups.remove(0);
    }
    let mut block = vec![
        format!("{i}struct KeepSymbolsList<const N: usize>([*const (); N]);{SECTION_START}"),
        format!("{i}unsafe impl<const N: usize> Sync for KeepSymbolsList<N> {{}}"),
    ];
    let last = groups.len() - 1;
    for (n, (cfg, exprs)) in groups.iter().enumerate() {
        block.extend(cfg.iter().map(|c| format!("{i}{c}")));
        block.push(format!("{i}#[unsafe(link_section = \"{}\")]", list_section(cpu)));
        block.push(format!("{i}#[used(linker)]"));
        let name = if n == 0 { "KEEP_SYMBOLS_LIST".to_string() } else { format!("KEEP_SYMBOLS_LIST_{n}") };
        block.push(format!("{i}static {name}: KeepSymbolsList<{}> = KeepSymbolsList([", exprs.len()));
        block.extend(exprs.iter().map(|e| format!("{i}    {e},")));
        block.push(format!("{i}]);{}", if n == last { SECTION_END } else { "" }));
    }
    block
}

pub fn normalize(text: &str, cpu: Cpu) -> Option<String> {
    let lines: Vec<&str> = text.split('\n').collect();
    section_range(&lines, directive_line(text).ok()??)?;
    let out = render(text, cpu, Some(&read(text)));
    (out != text).then_some(out)
}
