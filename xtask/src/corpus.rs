use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use anyhow::{Context, Result, bail, ensure};
use object::{Object, ObjectSection, ObjectSymbol, SymbolKind};
use regex::Regex;
use serde::Deserialize;

use crate::corpus_gen;
use crate::corpus_inline as inline;
use crate::corpus_keep as keep;
use crate::corpus_layout as layout;
use crate::corpus_mods::{Lines, Mods};

const REGIONS: [&str; 3] = ["us", "eu", "jp"];
const ARM9_FEATURES: [(&str, &str); 3] = [("us", "na"), ("eu", "eu"), ("jp", "jp")];
const LOCK_FILE: &str = "asm-corpus.lock";
const RUN_LOCK: &str = "target/xtask-corpus.lock";
const BLOCK_START: &str = "/*(ASM_START";
const BLOCK_END: &str = "ASM_END)*/";
const COMMAND_PREFIX: &str = "/*(asm-corpus-filename";
const STUB_PREFIX: &str = "/*(stub";
const OBJDUMP: &str = "arm-none-eabi-objdump";
const SKIP_DIRS: [&str; 4] = ["target", "xtask", ".git", "asm-corpus"];
const DEFAULT_CORPUS: &str = "asm-corpus";

const LIST_STYLE_MAX_LINES: usize = 25;

pub(crate) const MARK_TAG: &str = "\x01mark\t";
const ALIGN_MAX_CELLS: usize = 2_000_000;
const GAP_OPEN: i32 = 5;
const GAP_EXTEND: i32 = 0;

const CONDS: [&str; 17] = ["eq", "ne", "cs", "hs", "cc", "lo", "mi", "pl", "vs", "vc", "hi", "ls", "ge", "lt", "gt", "le", "al"];
const BRANCH_OPS: [&str; 4] = ["b", "bl", "bx", "blx"];
const COMPARE_OPS: [&str; 4] = ["cmp", "cmn", "tst", "teq"];
const ALU_OPS: [&str; 26] = [
    "mov", "mvn", "add", "adc", "sub", "sbc", "rsb", "rsc", "and", "orr", "eor", "bic", "lsl", "lsr", "asr", "ror", "rrx",
    "mul", "mla", "umull", "umlal", "smull", "smlal", "clz", "qadd", "qsub",
];
const LOAD_OPS: [&str; 13] = [
    "ldr", "ldrb", "ldrh", "ldrsb", "ldrsh", "ldrd", "ldm", "ldmia", "ldmib", "ldmda", "ldmdb", "ldmfd", "pop",
];
const STORE_OPS: [&str; 12] = ["str", "strb", "strh", "strd", "stm", "stmia", "stmib", "stmda", "stmdb", "stmfd", "push", "swp"];
const OTHER_OPS: [&str; 8] = ["msr", "mrs", "mcr", "mrc", "svc", "swi", "udf", "nop"];

const USAGE: &str = "usage: cargo xtask corpus [run|stub|lint|fix|move|keep-symbols-list [symbols...]|strip|sync|status|lock|generate|verify] [--corpus <asm-corpus dir>] \
                     [--decomp <pmd-sky clone with all three regions built>] [--no-sync] [--encodings] \
                     [--stub-mode in-place|position|position-with-warn] [--no-auto-keep] [keep-symbols-list --remove] [lint|fix [files...] [--fix-structure] [--fix-ordering] [--fix-all]] \
                     [--style <auto|list|columns>[,compact][,align][,label_last|label_first]] [--labels hide|before|after] [move --stage|--stage-strict] [status --group-by groups[+paths]]";

static COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    let list = r"(\[[^\]]*\]|[^,\s\)\]\{]+(?:\{[^}]*\})?)";
    Regex::new(&format!(
        r"/\*\(asm-corpus-filename:\s*{list}\s*,\s*objdump-symbol-name:\s*(.*?)\s*\)(DONE\(\s*'([^']*)'\s*((?:,\s*[A-Za-z]+\s*)*)\))?\*/"
    ))
    .unwrap()
});

static STUB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/\*\(stub\s+text:\s*([^)\s]+)\s*\)\*/").unwrap());
static LINK_SECTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"link_section\s*=\s*"([^"]+)""#).unwrap());
pub(crate) static INSN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([\s>]*[0-9a-f]+:)\t([0-9a-f ]+)\t(.*)$").unwrap());
pub(crate) static LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9a-f]+ <[^>]+>:$").unwrap());
static WORD_LOAD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\tldr(?:eq|ne|cs|hs|cc|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le|al)?\t[a-z0-9]+, \[pc(?:, #-?\d+)?\]\t@ \(?([0-9a-f]+)")
        .unwrap()
});
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\.word\t0x([0-9a-f]{8})$").unwrap());
static POOL_REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[pc(?:, #-?\d+)?\].*@ \(?([0-9a-f]+)").unwrap());

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub(crate) enum Cpu {
    Arm7,
    Arm9,
}

impl Cpu {
    pub(crate) const ALL: [Cpu; 2] = [Cpu::Arm7, Cpu::Arm9];

    pub(crate) fn cfg(self) -> &'static str {
        match self {
            Cpu::Arm7 => "armv4t",
            Cpu::Arm9 => "armv5te",
        }
    }

    fn of_path(rsdec: &Path, path: &Path) -> Option<Cpu> {
        Cpu::ALL.into_iter().find(|c| path.starts_with(rsdec.join(c.dir())))
    }

    pub(crate) fn split(s: &str) -> (Option<Cpu>, &str) {
        match Cpu::from_label(s) {
            Some((cpu, rest)) => (Some(cpu), rest),
            None => (None, s),
        }
    }

    fn from_label(s: &str) -> Option<(Cpu, &str)> {
        Cpu::ALL.into_iter().find_map(|cpu| {
            [format!("{}/", cpu.label()), format!("{}/", cpu.label().replace('_', "-"))]
                .iter()
                .find_map(|p| s.strip_prefix(p.as_str()))
                .map(|rest| (cpu, rest))
        })
    }

    pub(crate) fn dir(self) -> &'static str {
        match self {
            Cpu::Arm7 => "arm7",
            Cpu::Arm9 => "arm9",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Cpu::Arm7 => "arm7_bin",
            Cpu::Arm9 => "arm9_bin",
        }
    }

    fn feature(self, region: &str) -> Option<&'static str> {
        match self {
            Cpu::Arm7 => None,
            Cpu::Arm9 => ARM9_FEATURES.iter().find(|(r, _)| *r == region).map(|(_, f)| *f),
        }
    }
}

#[derive(Deserialize)]
struct Record {
    source: String,
    #[serde(default)]
    notes: Vec<String>,
    regions: BTreeMap<String, RegionEntry>,
}

#[derive(Deserialize)]
struct RegionEntry {
    addr: Option<String>,
    listing: Option<Vec<String>>,
    same_as: Option<String>,
}

impl Record {
    fn columns(&self) -> impl Iterator<Item = &'static str> + '_ {
        REGIONS
            .into_iter()
            .filter(|r| self.regions.get(*r).is_some_and(|e| e.listing.is_some()))
    }
}

#[derive(Deserialize)]
struct Ident {
    name: String,
    #[serde(default)]
    aliases: Vec<String>,
}

struct LockEntry {
    path: String,
    name: String,
    aliases: Vec<String>,
}

impl LockEntry {
    fn dir(&self) -> &str {
        self.path.rsplit_once('/').map_or("", |(d, _)| d)
    }

    fn stem(&self) -> &str {
        let file = self.path.rsplit_once('/').map_or(self.path.as_str(), |(_, f)| f);
        file.strip_suffix(".json").unwrap_or(file)
    }

    fn addr(&self) -> &str {
        let stem = self.stem();
        stem.split_once('_').map_or(stem, |(a, _)| a)
    }
}

struct Corpus {
    root: PathBuf,
    copies: BTreeMap<String, Vec<String>>,
}

impl Corpus {
    fn preferred_copy<'a>(&'a self, canon: &'a str) -> &'a str {
        let copies = &self.copies[canon];
        if copies.iter().any(|c| c == canon) { canon } else { &copies[0] }
    }
}

struct Source {
    path: PathBuf,
    rel: String,
    original: String,
    stripped: String,
    stubbed: bool,
    commands: Vec<RawCommand>,
}

#[derive(Clone)]
enum SymSpec {
    Plain(String),
    Inline(inline::InlineSpec),
}

impl SymSpec {
    fn keep_queries(&self) -> Vec<String> {
        match self {
            SymSpec::Plain(s) => vec![s.clone()],
            SymSpec::Inline(spec) => spec.callers().map(|c| spec.caller_query(c)).collect(),
        }
    }
}

struct RawCommand {
    line: usize,
    indent: String,
    units: Vec<(String, Mods)>,
    syms: Vec<(SymSpec, Lines)>,
    done: Option<(String, BTreeSet<String>)>,
}

struct Cmd {
    source: usize,
    line: usize,
    indent: String,
    units: Vec<(usize, Mods)>,
    syms: Vec<(SymSpec, Lines)>,
    done: Option<(String, BTreeSet<String>)>,
    state: State,
}

#[derive(Clone)]
enum State {
    Complete(String),
    Stage(&'static str),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Auto,
    List,
    Columns,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Labels {
    Hide,
    Before,
    After,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StubMode {
    InPlace,
    Position,
    PositionWithWarn,
}

#[derive(Clone, Copy)]
struct Options {
    stub_mode: StubMode,
    auto_keep: bool,
    encodings: bool,
    style: Style,
    compact: bool,
    align: bool,
    label_first: bool,
    labels: Labels,
    group_by_paths: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Run,
    Stub,
    Sync,
    Status,
}

enum Op {
    Move(String, String),
    Remove(String),
}

pub fn main(args: &[String]) -> Result<()> {
    let mut sub = None;
    let mut corpus = None;
    let mut decomp = None;
    let mut no_sync = false;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut symbols: Vec<String> = Vec::new();
    let mut remove = false;
    let mut stage = crate::corpus_move::Stage::No;
    let (mut fix_structure, mut fix_ordering) = (false, false);
    let mut opts = Options { stub_mode: StubMode::InPlace, auto_keep: true, encodings: false, style: Style::Auto, compact: false, align: false, label_first: false, labels: Labels::Hide, group_by_paths: false };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "help" | "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(());
            }
            "--corpus" => corpus = Some(PathBuf::from(it.next().context("--corpus needs a path")?)),
            "--decomp" => decomp = Some(PathBuf::from(it.next().context("--decomp needs a path")?)),
            "--no-sync" => no_sync = true,
            "--no-auto-keep" => opts.auto_keep = false,
            "--remove" => remove = true,
            "--stage" => stage = crate::corpus_move::Stage::IfClean,
            "--stage-strict" => stage = crate::corpus_move::Stage::Strict,
            "--fix-structure" => fix_structure = true,
            "--fix-ordering" => fix_ordering = true,
            "--fix-all" => (fix_structure, fix_ordering) = (true, true),
            "--stub-mode" => {
                opts.stub_mode = match it.next().map(String::as_str) {
                    Some("in-place") => StubMode::InPlace,
                    Some("position") => StubMode::Position,
                    Some("position-with-warn") => StubMode::PositionWithWarn,
                    other => bail!("--stub-mode expects in-place, position or position-with-warn, got {other:?}\n{USAGE}"),
                }
            }
            "--encodings" => opts.encodings = true,
            "--group-by" => {
                let spec = it.next().context("--group-by needs a value")?;
                let tokens: Vec<&str> = spec.split([',', '+']).map(str::trim).collect();
                for t in &tokens {
                    ensure!(matches!(*t, "groups" | "paths"), "unknown --group-by token '{t}' (expected groups, paths)\n{USAGE}");
                }
                ensure!(tokens.contains(&"groups"), "--group-by must include groups (e.g. --group-by groups+paths)");
                opts.group_by_paths = tokens.contains(&"paths");
            }
            "--style" => {
                let spec = it.next().context("--style needs a value")?;
                let mut layout = None;
                let mut label_order = None;
                for token in spec.split(',').map(str::trim) {
                    let l = match token {
                        "auto" => Style::Auto,
                        "list" => Style::List,
                        "columns" => Style::Columns,
                        "compact" => {
                            opts.compact = true;
                            continue;
                        }
                        "align" => {
                            opts.align = true;
                            continue;
                        }
                        "label_last" | "label_first" => {
                            ensure!(
                                label_order.replace(token == "label_first").is_none(),
                                "--style takes at most one of label_last, label_first"
                            );
                            continue;
                        }
                        other => bail!(
                            "unknown --style token '{other}' (expected auto, list, columns, compact, align, label_last, label_first)\n{USAGE}"
                        ),
                    };
                    ensure!(layout.replace(l).is_none(), "--style takes at most one of auto, list, columns");
                }
                opts.style = layout.unwrap_or(Style::Auto);
                opts.label_first = label_order.unwrap_or(false);
            }
            "--labels" => {
                opts.labels = match it.next().map(String::as_str) {
                    Some("hide") => Labels::Hide,
                    Some("before") => Labels::Before,
                    Some("after") => Labels::After,
                    other => bail!("--labels expects hide, before or after, got {other:?}\n{USAGE}"),
                }
            }
            s if !s.starts_with('-') && sub.is_none() => sub = Some(s.to_string()),
            s if !s.starts_with('-') && matches!(sub.as_deref(), Some("lint" | "fix")) => files.push(PathBuf::from(s)),
            s if !s.starts_with('-') && sub.as_deref() == Some("keep-symbols-list") => symbols.push(s.to_string()),
            s => bail!("unexpected argument '{s}'\n{USAGE}"),
        }
    }

    let rsdec = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    fs::create_dir_all(rsdec.join("target"))?;
    let run_lock = fs::File::create(rsdec.join(RUN_LOCK))?;
    run_lock.lock()?;

    let corpus = corpus.unwrap_or_else(|| rsdec.join(DEFAULT_CORPUS));
    let decomp = || decomp.clone().with_context(|| format!("--decomp <pmd-sky clone with all three regions built> is required\n{USAGE}"));
    match sub.as_deref().unwrap_or("run") {
        "strip" => strip_all(&rsdec),
        "lock" => make_lock(&rsdec, &corpus),
        "run" => pipeline(&rsdec, &corpus, Mode::Run, opts, false),
        "sync" => pipeline(&rsdec, &corpus, Mode::Sync, opts, false),
        "status" => pipeline(&rsdec, &corpus, Mode::Status, opts, false),
        "stub" => pipeline(&rsdec, &corpus, Mode::Stub, opts, false),
        "keep-symbols-list" => keep_cmd(&rsdec, &symbols, remove),
        "generate" => generate(&rsdec, &decomp()?, &corpus, no_sync, opts),
        "verify" => corpus_gen::verify(&decomp()?, &corpus),
        "lint" => lint_cmd(&rsdec, &files),
        "move" => crate::corpus_move::run(&rsdec, &source_files(&rsdec)?, stage),
        "fix" => {
            if !fix_structure && !fix_ordering {
                fix_structure = true;
            }
            fix_cmd(&rsdec, &files, fix_structure, fix_ordering)
        }
        s => bail!("unknown subcommand '{s}'\n{USAGE}"),
    }
}

fn generate(rsdec: &Path, decomp: &Path, corpus: &Path, no_sync: bool, opts: Options) -> Result<()> {
    let name = corpus.file_name().context("--corpus has no final path component")?.to_string_lossy();
    let staging = corpus.with_file_name(format!("{name}.new"));
    let is_generated = |dir: &Path| -> Result<bool> {
        Ok(!dir.exists() || dir.join(corpus_gen::MARKER_FILE).exists() || fs::read_dir(dir)?.next().is_none())
    };
    for dir in [&staging, corpus] {
        ensure!(
            is_generated(dir)?,
            "refusing to replace {}: it is not empty and has no {} marker (not a generated corpus)",
            dir.display(),
            corpus_gen::MARKER_FILE
        );
    }
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    let written = corpus_gen::generate(decomp, &staging)?;
    if corpus.exists() {
        fs::remove_dir_all(corpus)?;
    }
    fs::rename(&staging, corpus)?;
    println!("wrote {written} function files to {}", corpus.display());
    if no_sync {
        return Ok(());
    }
    make_lock(rsdec, corpus)?;
    pipeline(rsdec, corpus, Mode::Sync, opts, false)
}

fn walk(dir: &Path, skip: &[&str], out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if !skip.iter().any(|s| entry.file_name() == *s) {
                walk(&path, skip, out)?;
            }
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn rel_str(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap()
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn canonical(rel: &str) -> Option<String> {
    let p: Vec<&str> = rel.split('/').collect();
    match p.len() {
        3 => Some(rel.to_string()),
        5 if p[2] == "DONE" => Some(format!("{}/{}/{}", p[0], p[1], p[4])),
        _ => None,
    }
}

fn done_path(canon: &str, group: &str) -> String {
    let (dir, file) = canon.rsplit_once('/').unwrap();
    format!("{dir}/DONE/{group}/{file}")
}

fn walk_corpus(root: &Path) -> Result<Corpus> {
    let mut files = Vec::new();
    for family in ["arm7", "arm9"] {
        walk(&root.join(family), &[], &mut files)?;
    }
    let mut copies: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in files {
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let rel = rel_str(root, &path);
        let canon = canonical(&rel).with_context(|| format!("unexpected corpus file location {rel}"))?;
        copies.entry(canon).or_default().push(rel);
    }
    Ok(Corpus { root: root.to_path_buf(), copies })
}

fn make_lock(rsdec: &Path, corpus_root: &Path) -> Result<()> {
    let corpus = walk_corpus(corpus_root)?;
    let mut lines = Vec::new();
    for canon in corpus.copies.keys() {
        let path = corpus.root.join(corpus.preferred_copy(canon));
        let ident: Ident = serde_json::from_slice(&fs::read(&path)?).with_context(|| format!("parsing {}", path.display()))?;
        lines.push(format!("{canon}\t{}\t{}", ident.name, ident.aliases.join(",")));
    }
    fs::write(rsdec.join(LOCK_FILE), lines.join("\n") + "\n")?;
    println!("wrote {} entries to {LOCK_FILE}", lines.len());
    Ok(())
}

fn read_lock(rsdec: &Path) -> Result<Vec<LockEntry>> {
    let text = fs::read_to_string(rsdec.join(LOCK_FILE))
        .with_context(|| format!("reading {LOCK_FILE}; generate it with `cargo xtask corpus lock --corpus <dir>`"))?;
    text.lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let mut f = l.split('\t');
            let (Some(path), Some(name), Some(aliases)) = (f.next(), f.next(), f.next()) else {
                bail!("malformed {LOCK_FILE} line: {l}");
            };
            Ok(LockEntry {
                path: path.to_string(),
                name: name.to_string(),
                aliases: aliases.split(',').filter(|a| !a.is_empty()).map(String::from).collect(),
            })
        })
        .collect()
}

fn strip_blocks(text: &str) -> Result<String, usize> {
    let mut out = Vec::new();
    let mut open = None;
    for (i, line) in text.lines().enumerate() {
        if open.is_none() && line.trim_start().starts_with(BLOCK_START) {
            open = Some(i);
        }
        if open.is_some() {
            if line.trim_end().ends_with(BLOCK_END) {
                open = None;
            }
            continue;
        }
        out.push(line);
    }
    if let Some(i) = open {
        return Err(i);
    }
    let mut s = out.join("\n");
    if text.ends_with('\n') && !out.is_empty() {
        s.push('\n');
    }
    Ok(s)
}

fn insert_blocks(text: &str, blocks: &BTreeMap<usize, Vec<String>>) -> String {
    let mut out: Vec<&str> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        out.push(line);
        if let Some(b) = blocks.get(&i) {
            out.extend(b.iter().map(String::as_str));
        }
    }
    let mut s = out.join("\n");
    if text.ends_with('\n') && !out.is_empty() {
        s.push('\n');
    }
    s
}

fn line_of(text: &str, byte: usize) -> usize {
    text[..byte].matches('\n').count()
}

fn parse_list(s: &str) -> Result<Vec<(String, Mods)>, String> {
    let items: Vec<&str> = match s.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        Some(inner) => split_top(inner),
        None => vec![s.trim()],
    };
    let mut out = Vec::new();
    for item in items {
        let (name, mods) = Mods::split(item)?;
        if !name.is_empty() && name != "none" {
            out.push((name.to_string(), mods));
        }
    }
    Ok(out)
}

pub(crate) fn split_top(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for (i, ch) in s.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out
}

fn parse_syms(s: &str) -> Result<Vec<(SymSpec, Lines)>, String> {
    let s = s.trim();
    let items = match s.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        Some(inner) if split_top(s).len() == 1 => split_top(inner),
        _ => split_top(s),
    };
    if items.len() > 1 && !s.starts_with('[') {
        return Err(format!("'{s}': write multiple symbols as a [...] list"));
    }
    let mut out = Vec::new();
    for whole in items {
        let (item, mods) = Mods::split(whole)?;
        if item.is_empty() || item == "none" {
            continue;
        }
        if mods.ignore {
            return Err(format!("'{whole}': ignore only applies to asm-corpus-filename items"));
        }
        let spec = if let Some(inner) = item.strip_prefix("inline(").and_then(|r| r.strip_suffix(')')) {
            SymSpec::Inline(inline::InlineSpec::parse(inner)?)
        } else if item.contains(|c: char| c.is_whitespace() || "()[],".contains(c)) {
            return Err(format!("'{item}' is not a symbol name"));
        } else {
            SymSpec::Plain(item.to_string())
        };
        out.push((spec, mods.lines));
    }
    Ok(out)
}

fn parse_commands(rel: &str, text: &str, errors: &mut Vec<String>) -> Vec<RawCommand> {
    let mut starts = HashSet::new();
    let mut out = Vec::new();
    for c in COMMAND.captures_iter(text) {
        let m = c.get(0).unwrap();
        starts.insert(m.start());
        let start_line = line_of(text, m.start());
        let indent: String = text
            .lines()
            .nth(start_line)
            .unwrap_or("")
            .chars()
            .take_while(|c| c.is_whitespace())
            .collect();
        let done = match c.get(3) {
            None => None,
            Some(_) => {
                let group = c[4].trim().to_string();
                if group.is_empty() || group == "." || group == ".." || group.contains(['/', '\\']) {
                    errors.push(format!("{rel}:{}: invalid DONE group name '{group}'", start_line + 1));
                }
                let mut regions = BTreeSet::new();
                for r in c[5].split(',').map(str::trim).filter(|r| !r.is_empty()) {
                    if REGIONS.contains(&r) {
                        regions.insert(r.to_string());
                    } else {
                        errors.push(format!("{rel}:{}: unknown region '{r}' (expected us, eu, jp)", start_line + 1));
                    }
                }
                Some((group, regions))
            }
        };
        out.push(RawCommand {
            line: line_of(text, m.end() - 1),
            indent,
            units: parse_list(&c[1]).unwrap_or_else(|e| {
                errors.push(format!("{rel}:{}: {e}", start_line + 1));
                Vec::new()
            }),
            syms: parse_syms(&c[2]).unwrap_or_else(|e| {
                errors.push(format!("{rel}:{}: {e}", start_line + 1));
                Vec::new()
            }),
            done,
        });
    }
    for (i, _) in text.match_indices(COMMAND_PREFIX) {
        if !starts.contains(&i) {
            errors.push(format!("{rel}:{}: malformed command comment", line_of(text, i) + 1));
        }
    }
    out
}

fn load_sources(rsdec: &Path, errors: &mut Vec<String>, mut stubs: Option<&mut Stubs>) -> Result<Vec<Source>> {
    let files: Vec<(PathBuf, String, String)> = source_files(rsdec)?
        .into_iter()
        .map(|path| {
            let rel = rel_str(rsdec, &path);
            let text = fs::read_to_string(&path).with_context(|| format!("reading {rel}"))?;
            Ok((path, rel, text))
        })
        .collect::<Result<_>>()?;
    if let Some(st) = stubs.as_deref_mut() {
        for (_, rel, text) in &files {
            st.note_sections(rel, text);
        }
    }
    let mut out = Vec::new();
    for (path, rel, original) in files {
        let stripped = match strip_blocks(&original) {
            Ok(s) => s,
            Err(line) => {
                errors.push(format!("{rel}:{}: {BLOCK_START} without {BLOCK_END}", line + 1));
                continue;
            }
        };
        let expanded = match stubs.as_deref_mut() {
            Some(st) => st.expand(&rel, &stripped, errors)?,
            None => stripped.clone(),
        };
        let stubbed = expanded != stripped;
        let commands = parse_commands(&rel, &expanded, errors);
        out.push(Source { path, rel, original, stripped: expanded, stubbed, commands });
    }
    Ok(out)
}

struct Stubs<'a> {
    lock: &'a [LockEntry],
    corpus: &'a Corpus,
    mode: StubMode,
    seen: HashMap<String, String>,
    written: usize,
    warnings: Vec<String>,
    modules: HashMap<String, Vec<BTreeMap<String, u64>>>,
}

fn region_addrs(rec: &Record) -> BTreeMap<String, u64> {
    rec.regions
        .iter()
        .filter_map(|(r, e)| Some((r.clone(), u64::from_str_radix(e.addr.as_deref()?.trim_start_matches("0x"), 16).ok()?)))
        .collect()
}

impl Stubs<'_> {
    fn note_sections(&mut self, rel: &str, text: &str) {
        for c in LINK_SECTION.captures_iter(text) {
            let at = format!("{rel}:{}", line_of(text, c.get(0).unwrap().start()) + 1);
            self.seen.entry(c[1].to_string()).or_insert(at);
        }
    }

    fn expand(&mut self, rel: &str, text: &str, errors: &mut Vec<String>) -> Result<String> {
        let starts: HashSet<usize> = STUB.find_iter(text).map(|m| m.start()).collect();
        for (i, _) in text.match_indices(STUB_PREFIX) {
            if !starts.contains(&i) {
                errors.push(format!("{rel}:{}: malformed stub comment", line_of(text, i) + 1));
            }
        }
        let mut text = text.to_string();
        let mut skipped = 0;
        loop {
            let Some(c) = STUB.captures_iter(&text).nth(skipped) else { break };
            let (m, query) = (c.get(0).unwrap().range(), c[1].to_string());
            let line = line_of(&text, m.start);
            let spec = match self.stub(&query, &format!("{rel}:{}", line + 1))? {
                Ok(spec) => spec,
                Err(e) => {
                    errors.push(format!("{rel}:{}: {e}", line + 1));
                    skipped += 1;
                    continue;
                }
            };
            let indent: String = text.split('\n').nth(line).unwrap_or("").chars().take_while(|c| c.is_whitespace()).collect();
            let in_place = |text: &str, warn: bool| {
                let body = stub_lines(&spec, warn);
                let joined = body.iter().enumerate().map(|(i, l)| if i == 0 { l.clone() } else { format!("{indent}{l}") });
                format!("{}{}{}", &text[..m.start], joined.collect::<Vec<_>>().join("\n"), &text[m.end..])
            };
            if self.mode == StubMode::InPlace {
                text = in_place(&text, false);
                self.written += 1;
                continue;
            }
            let removed = remove_span(&text, m.clone());
            match layout::position(&removed, &spec.section, spec.arm9) {
                Some(p) => {
                    text = layout::insert_at(&removed, &p, &stub_lines(&spec, false));
                    self.written += 1;
                }
                None if self.mode == StubMode::PositionWithWarn => {
                    text = in_place(&text, true);
                    self.written += 1;
                    self.warnings.push(format!("{rel}:{}: stub for {} expanded in place: positioning failed", line + 1, spec.name));
                }
                None => {
                    self.seen.remove(&spec.section);
                    self.warnings.push(format!(
                        "{rel}:{}: stub for {} left unexpanded: no surrounding link_sections found to position against",
                        line + 1,
                        spec.name
                    ));
                    skipped += 1;
                }
            }
        }
        Ok(text)
    }

    fn module(&mut self, dir: &str) -> Result<&[BTreeMap<String, u64>]> {
        if !self.modules.contains_key(dir) {
            let mut all = Vec::new();
            for e in self.lock.iter().filter(|e| e.dir() == dir) {
                let path = self.corpus.root.join(self.corpus.preferred_copy(&e.path));
                let rec: Record =
                    serde_json::from_slice(&fs::read(&path)?).with_context(|| format!("parsing {}", path.display()))?;
                all.push(region_addrs(&rec));
            }
            self.modules.insert(dir.to_string(), all);
        }
        Ok(&self.modules[dir])
    }

    fn anchor(&mut self, dir: &str, region: &str, addr: u64) -> Result<Option<u64>> {
        Ok(self
            .module(dir)?
            .iter()
            .filter_map(|m| Some((*m.get(region)?, *m.get("us")?)))
            .filter(|(ra, _)| *ra < addr)
            .max_by_key(|(ra, _)| *ra)
            .map(|(_, us)| us))
    }

    fn stub(&mut self, query: &str, at: &str) -> Result<Result<StubSpec, String>> {
        let u = match resolve_unit(self.lock, query) {
            Ok(u) => u,
            Err(e) => return Ok(Err(e)),
        };
        let e = &self.lock[u];
        let prefix = match e.dir() {
            "arm9/main" | "arm7/static" => ".text.origaddr.",
            "arm9/itcm" => ".text.itcm.origaddr.",
            "arm7/wram" => ".text.wram.origaddr.",
            "arm7/main" => ".text.main.origaddr.",
            other => return Ok(Err(format!("stubs for {other} are not supported yet"))),
        };
        let path = self.corpus.root.join(self.corpus.preferred_copy(&e.path));
        let rec: Record = serde_json::from_slice(&fs::read(&path)?).with_context(|| format!("parsing {}", path.display()))?;
        let addr_in = |r: &str| {
            rec.regions.get(r).and_then(|x| x.addr.as_deref()).and_then(|a| u64::from_str_radix(a.trim_start_matches("0x"), 16).ok())
        };
        let placed = addr_in("us").or_else(|| addr_in("eu")).or_else(|| addr_in("jp")).unwrap_or(0);
        if e.dir() == "arm9/main" && placed < 0x0200_0800 {
            return Ok(Err(format!(
                "{} is in the secure area (0x02000000..0x02000800), which is defined by secure_area.s",
                e.name
            )));
        }
        let token = match addr_in("us") {
            Some(a) => format!("{a:08X}"),
            None => {
                let Some((r, a)) = ["eu", "jp"].into_iter().find_map(|r| addr_in(r).map(|a| (r, a))) else {
                    return Ok(Err(format!("{} has no address in any region", e.path)));
                };
                let (dir, name) = (e.dir().to_string(), e.name.clone());
                let Some(us) = self.anchor(&dir, r, a)? else {
                    return Ok(Err(format!("{name} has no preceding function in {r} that also exists in us to anchor to")));
                };
                if dir == "arm9/main" && us < 0x0200_0800 {
                    return Ok(Err(format!("{name}'s anchor 0x{us:08X} is in the secure area")));
                }
                format!("{us:08X}{r}.{a:08X}")
            }
        };
        let section = format!("{prefix}{token}");
        if let Some(existing) = self.seen.get(&section) {
            return Ok(Err(format!("link_section \"{section}\" already exists at {existing}")));
        }
        self.seen.insert(section.clone(), format!("{at} (stub)"));
        Ok(Ok(StubSpec { section, name: e.name.clone(), arm9: e.dir().starts_with("arm9/") }))
    }
}

struct StubSpec {
    section: String,
    name: String,
    arm9: bool,
}

fn stub_lines(spec: &StubSpec, warn: bool) -> Vec<String> {
    let mut out = Vec::new();
    if warn {
        out.push(layout::POSITION_WARN.to_string());
    }
    out.extend([
        "#[inline(never)]".to_string(),
        format!("#[unsafe(link_section = \"{}\")]", spec.section),
        "#[unsafe(no_mangle)]".to_string(),
        format!("pub extern \"C\" fn {}() {{", spec.name),
        "    ".to_string(),
        "}".to_string(),
    ]);
    out
}

fn remove_span(text: &str, span: std::ops::Range<usize>) -> String {
    let line_start = text[..span.start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[span.end..].find('\n').map_or(text.len(), |i| span.end + i);
    if text[line_start..span.start].trim().is_empty() && text[span.end..line_end].trim().is_empty() {
        let cut_end = if line_end < text.len() { line_end + 1 } else { line_end };
        format!("{}{}", &text[..line_start], &text[cut_end..])
    } else {
        format!("{}{}", &text[..span.start], &text[span.end..])
    }
}

pub(crate) fn warn(msg: &str) {
    anstream::println!("\x1b[33mWARN:\x1b[0m {msg}");
}

fn lint_targets(rsdec: &Path, files: &[PathBuf]) -> Result<Vec<(PathBuf, String)>> {
    if files.is_empty() {
        return Ok(source_files(rsdec)?
            .into_iter()
            .filter_map(|p| {
                let text = fs::read_to_string(&p).ok()?;
                COMMAND.is_match(&text).then_some(p)
            })
            .map(|p| {
                let rel = rel_str(rsdec, &p);
                (p, rel)
            })
            .collect());
    }
    files
        .iter()
        .map(|f| {
            let p = fs::canonicalize(f).with_context(|| format!("no such file {}", f.display()))?;
            let rel = p.strip_prefix(rsdec).map_or_else(|_| p.display().to_string(), |r| r.to_string_lossy().into_owned());
            Ok((p, rel))
        })
        .collect()
}

fn lint_text(rel: &str, text: &str) -> usize {
    let found = layout::lint(text, rel.ends_with(".rs"));
    for (line, msg) in &found {
        warn(&format!("{rel}:{}: {msg}", line + 1));
    }
    found.len()
}

fn lint_cmd(rsdec: &Path, files: &[PathBuf]) -> Result<()> {
    let mut total = 0;
    let targets = lint_targets(rsdec, files)?;
    for (path, rel) in &targets {
        total += lint_text(rel, &fs::read_to_string(path)?);
    }
    println!("lint: {} files, {total} warnings", targets.len());
    Ok(())
}

fn fix_cmd(rsdec: &Path, files: &[PathBuf], structure: bool, ordering: bool) -> Result<()> {
    let targets = lint_targets(rsdec, files)?;
    let mut changed = 0;
    for (path, rel) in &targets {
        let original = fs::read_to_string(path)?;
        let mut text = original.clone();
        if structure {
            match layout::fix_structure(&text) {
                Some(t) => text = t,
                None => warn(&format!("{rel}: cfg_select layout not recognized; structure left as is")),
            }
        }
        if ordering {
            let (t, problems) = layout::fix_ordering(&text);
            text = t;
            for p in problems {
                warn(&format!("{rel}: {p}"));
            }
        }
        if text != original && write_if_unchanged(path, rel, &original, &text)? {
            changed += 1;
        }
        lint_text(rel, &text);
    }
    println!("fix: {} files checked, {changed} rewritten", targets.len());
    Ok(())
}

fn resolve_unit(lock: &[LockEntry], query: &str) -> Result<usize, String> {
    let q = query.strip_suffix(".json").unwrap_or(query);
    let (qual, key) = q.rsplit_once('/').unwrap_or(("", q));
    let hits: Vec<usize> = lock
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            (qual.is_empty() || e.dir() == qual || e.dir().starts_with(&format!("{qual}/")))
                && (e.stem() == key || e.name == key || e.aliases.iter().any(|a| a == key) || e.addr().eq_ignore_ascii_case(key))
        })
        .map(|(i, _)| i)
        .collect();
    match hits.as_slice() {
        [i] => Ok(*i),
        [] => Err(format!("no asm-corpus entry matches '{query}'")),
        _ => {
            let by_name: Vec<String> = hits.iter().map(|&i| format!("{}/{}", lock[i].dir(), lock[i].name)).collect();
            let unique = by_name.iter().collect::<HashSet<_>>().len() == by_name.len();
            let names: Vec<String> = if unique {
                by_name
            } else {
                hits.iter().map(|&i| format!("{}/{}", lock[i].dir(), lock[i].stem())).collect()
            };
            Err(format!("asm-corpus name '{query}' is ambiguous; qualify as one of: {}", names.join(", ")))
        }
    }
}

fn state_of(records: &[&Record], done: &Option<(String, BTreeSet<String>)>) -> State {
    let columns: BTreeSet<&'static str> = records.iter().flat_map(|r| r.columns()).collect();
    let done_regions = done.as_ref().map(|(_, s)| s);
    if let Some((group, regions)) = done
        && (columns.len() <= 1 || columns.iter().all(|c| regions.contains(*c)))
    {
        return State::Complete(group.clone());
    }
    let stage = REGIONS
        .into_iter()
        .find(|r| columns.contains(r) && !done_regions.is_some_and(|s| s.contains(*r)))
        .unwrap_or("us");
    State::Stage(stage)
}

struct Builds {
    rsdec: PathBuf,
    indexes: HashMap<Cpu, keep::Index>,
    elves: HashMap<(Cpu, Option<&'static str>), PathBuf>,
    tables: HashMap<(Cpu, Option<&'static str>), HashSet<String>>,
}

impl Builds {
    fn elf(&mut self, cpu: Cpu, region: &str) -> Result<PathBuf> {
        let feature = cpu.feature(region);
        if let Some(p) = self.elves.get(&(cpu, feature)) {
            return Ok(p.clone());
        }
        let mut cmd = Command::new("cargo");
        cmd.current_dir(self.rsdec.join(cpu.dir()))
            .args(["build", "--release", "--message-format=json-render-diagnostics"])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if let Some(f) = feature
            && f != cpu.feature("us").unwrap()
        {
            cmd.args(["--no-default-features", "--features", f, "--target-dir", &format!("target/xtask-{f}")]);
        }
        let out = cmd.output().with_context(|| format!("running cargo build for {}", cpu.label()))?;
        ensure!(out.status.success(), "cargo build failed for {} ({})", cpu.label(), feature.unwrap_or("default"));
        let mut elf = None;
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            if v["reason"] == "compiler-artifact"
                && let Some(exe) = v["executable"].as_str()
            {
                elf = Some(PathBuf::from(exe));
            }
        }
        let elf = elf.with_context(|| format!("cargo build for {} produced no executable", cpu.label()))?;
        self.elves.insert((cpu, feature), elf.clone());
        Ok(elf)
    }

    fn table(&mut self, cpu: Cpu, region: &str) -> Result<&HashSet<String>> {
        let key = (cpu, cpu.feature(region));
        if !self.tables.contains_key(&key) {
            let elf = self.elf(cpu, region)?;
            let data = fs::read(&elf)?;
            let file = object::File::parse(&*data)?;
            let names = file
                .symbols()
                .filter(|s| s.is_definition())
                .filter_map(|s| s.name().ok().map(String::from))
                .collect();
            self.tables.insert(key, names);
        }
        Ok(&self.tables[&key])
    }

    fn resolve_sym(&mut self, query: &str, texts: &HashMap<PathBuf, String>) -> Result<Result<(Cpu, String), String>> {
        if let Some((cpu, rest)) = Cpu::from_label(query) {
            return Ok(Ok((cpu, rest.to_string())));
        }
        let mut found = Vec::new();
        for cpu in Cpu::ALL {
            if self.table(cpu, "us")?.contains(query) {
                found.push(cpu);
                continue;
            }
            if !self.indexes.contains_key(&cpu) {
                self.indexes.insert(cpu, keep::Binary::load(&self.rsdec, cpu)?.index(texts)?);
            }
            if self.indexes[&cpu].resolves(query) {
                found.push(cpu);
            }
        }
        Ok(match found.as_slice() {
            [cpu] => Ok((*cpu, query.to_string())),
            [] => Err(format!("symbol '{query}' not found in arm7_bin or arm9_bin")),
            _ => Err(format!("symbol '{query}' is in both binaries; qualify as arm7_bin/{query} or arm9_bin/{query}")),
        })
    }
}

fn objdump_range(elf: &Path, start: u64, stop: u64) -> Result<Vec<String>> {
    let out = Command::new(OBJDUMP)
        .arg("-d")
        .arg(format!("--start-address={start:#x}"))
        .arg(format!("--stop-address={stop:#x}"))
        .arg(elf)
        .output()
        .with_context(|| format!("running {OBJDUMP}"))?;
    ensure!(out.status.success(), "{OBJDUMP} failed: {}", String::from_utf8_lossy(&out.stderr));
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| LABEL.is_match(l) || INSN.is_match(l))
        .map(String::from)
        .collect())
}

fn code_labels(file: &object::File) -> Vec<(u64, String, String)> {
    let Some(data) = file.section_by_name(".code_labels").and_then(|s| s.data().ok()) else {
        return Vec::new();
    };
    let cstr = |off: usize| -> Option<(String, usize)> {
        let len = data.get(off..)?.iter().position(|&b| b == 0)?;
        Some((String::from_utf8_lossy(&data[off..off + len]).into_owned(), off + len + 1))
    };
    let mut out = Vec::new();
    let mut off = 0;
    while off + 4 <= data.len() {
        let addr = u32::from_le_bytes(data[off..off + 4].try_into().unwrap());
        let Some((id, next)) = cstr(off + 4) else { break };
        let Some((comment, next)) = cstr(next) else { break };
        off = (next + 3) & !3;
        if addr != 0 {
            out.push((u64::from(addr & !1), id, comment));
        }
    }
    out
}

fn insert_labels(lines: Vec<String>, mut pending: Vec<(u64, String, String)>, mode: Labels) -> Vec<String> {
    let tag = |(_, id, comment): (u64, String, String)| format!("{MARK_TAG}{id}\t{comment}");
    let mut out = Vec::new();
    for line in lines {
        let addr = insn_addr(&line);
        let here: Vec<String> = match addr {
            Some(a) => {
                let (hit, rest): (Vec<_>, Vec<_>) = pending.into_iter().partition(|(la, _, _)| *la == a);
                pending = rest;
                hit.into_iter().map(tag).collect()
            }
            None => Vec::new(),
        };
        if mode == Labels::Before {
            out.extend(here);
            out.push(line);
        } else {
            out.push(line);
            out.extend(here);
        }
    }
    out.extend(pending.into_iter().map(tag));
    out
}

fn disassemble(elf: &Path, sym: &str, labels: Labels) -> Result<Vec<String>> {
    let data = fs::read(elf)?;
    let file = object::File::parse(&*data)?;
    let range = symbol_range(&file, sym).with_context(|| {
        format!(
            "symbol '{sym}' not found (or has size 0) in {} (a function nothing calls yet is removed by --gc-sections)",
            elf.display()
        )
    })?;
    disassemble_ranges(elf, &file, &[range], labels, &[], Some(range.1))
}

fn symbol_range(file: &object::File, sym: &str) -> Option<(u64, u64)> {
    let s = file.symbols().find(|s| s.is_definition() && s.name() == Ok(sym))?;
    let start = s.address() & !1;
    (s.size() > 0).then_some((start, start + s.size()))
}

fn in_ranges(ranges: &[(u64, u64)], addr: u64) -> bool {
    ranges.iter().any(|(lo, hi)| (*lo..*hi).contains(&addr))
}

fn insn_addr(line: &str) -> Option<u64> {
    INSN.captures(line).and_then(|c| u64::from_str_radix(c[1].trim().trim_start_matches('>').trim_end_matches(':'), 16).ok())
}

fn disassemble_ranges(
    elf: &Path,
    file: &object::File,
    ranges: &[(u64, u64)],
    labels: Labels,
    marks: &[(u64, u64)],
    labels_end: Option<u64>,
) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for (i, &(lo, hi)) in ranges.iter().enumerate() {
        if i > 0 {
            lines.push("...".to_string());
        }
        lines.extend(objdump_range(elf, lo, hi)?);
    }
    let end = ranges.last().map_or(0, |r| r.1);
    let targets: BTreeSet<u64> = lines
        .iter()
        .filter_map(|l| POOL_REF.captures(l))
        .filter_map(|c| u64::from_str_radix(&c[1], 16).ok())
        .filter(|t| !in_ranges(ranges, *t))
        .collect();
    let mut runs: Vec<(u64, u64)> = Vec::new();
    for t in targets {
        match runs.last_mut() {
            Some((_, stop)) if *stop >= t => *stop = (*stop).max(t + 4),
            _ => runs.push((t, t + 4)),
        }
    }
    for (lo, hi) in runs {
        if lo != end {
            lines.push("...".to_string());
        }
        lines.extend(objdump_range(elf, lo, hi)?.into_iter().filter(|l| INSN.is_match(l)));
    }

    let mut names: HashMap<u64, BTreeSet<String>> = HashMap::new();
    for s in file.symbols() {
        if s.is_undefined() || matches!(s.kind(), SymbolKind::Section | SymbolKind::File) {
            continue;
        }
        if let Ok(n) = s.name()
            && !n.is_empty()
            && !n.starts_with('$')
            && !n.starts_with(".L")
        {
            names.entry(s.address()).or_default().insert(format!("{:#}", rustc_demangle::demangle(n)));
        }
    }
    let read_word = |addr: u64| -> Option<u32> {
        if addr % 4 != 0 {
            return None;
        }
        file.sections().find_map(|sec| {
            let (base, size) = (sec.address(), sec.size());
            if base == 0 || addr < base || addr + 4 > base + size {
                return None;
            }
            let data = sec.data().ok()?;
            let off = (addr - base) as usize;
            data.get(off..off + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        })
    };
    let lines = annotate_loads(lines, |addr| {
        let value = read_word(addr)?;
        Some(match names.get(&u64::from(value)) {
            Some(n) => format!("{value:#010x} <{}>", n.iter().cloned().collect::<Vec<_>>().join(", ")),
            None => format!("{value:#010x}"),
        })
    });
    let lines = match labels {
        Labels::Hide => lines,
        mode => {
            let inside = code_labels(file).into_iter().filter(|(a, _, _)| in_ranges(ranges, *a) || labels_end == Some(*a));
            insert_labels(lines, inside.collect(), mode)
        }
    };
    Ok(lines
        .into_iter()
        .map(|line| match insn_addr(&line) {
            Some(a) if in_ranges(marks, a) => match line.strip_prefix(' ') {
                Some(rest) => format!(">{rest}"),
                None => format!(">{line}"),
            },
            _ => line,
        })
        .collect())
}

type DwarfCache = HashMap<(Cpu, Option<&'static str>), inline::Info>;

fn inline_lines(
    builds: &mut Builds,
    dwarf: &mut DwarfCache,
    spec: &inline::InlineSpec,
    stage: &str,
    labels: Labels,
    at: &str,
    warnings: &mut Vec<String>,
) -> Result<Vec<String>> {
    let name = Cpu::split(&spec.target).1;
    let candidates = spec.cpu_hint().map_or(Cpu::ALL.to_vec(), |cpu| vec![cpu]);
    for &cpu in &candidates {
        if let std::collections::hash_map::Entry::Vacant(v) = dwarf.entry((cpu, cpu.feature(stage))) {
            let elf = builds.elf(cpu, stage)?;
            v.insert(inline::Info::load(&fs::read(&elf)?).with_context(|| elf.display().to_string())?);
        }
    }
    let head = |cpu: Cpu| format!("== {}/{name} ({}), INLINE", cpu.label(), cpu.feature(stage).unwrap_or("default"));
    let mut found = Vec::new();
    for &cpu in &candidates {
        if let Some(target) = dwarf[&(cpu, cpu.feature(stage))].find(name).map_err(|e| anyhow::anyhow!("{}: {e}", cpu.label()))? {
            found.push((cpu, head(cpu), target));
        }
    }
    match found.as_slice() {
        [] => {
            let (head, place) = match candidates.as_slice() {
                [cpu] => (head(*cpu), cpu.label()),
                _ => (format!("== {name}, INLINE"), "arm7_bin or arm9_bin"),
            };
            warnings.push(format!("{at}: '{name}' is not inlined anywhere in {place}"));
            Ok(vec![head, format!("-- not inlined anywhere in {place}: nothing linked calls it yet, or the name is misspelled")])
        }
        [(cpu, head, target)] => {
            let elf = builds.elf(*cpu, stage)?;
            let data = fs::read(&elf)?;
            let file = object::File::parse(&*data)?;
            let mut view = InlineView { elf: &elf, file: &file, head, labels, at, target, out: Vec::new(), warnings };
            for item in &spec.items {
                for pick in target.select(&item.selector) {
                    view.render(pick, item)?;
                }
            }
            Ok(view.out)
        }
        _ => bail!("'{name}' is inlined in both binaries; qualify as inline(=arm7_bin/{name}) or inline(=arm9_bin/{name})"),
    }
}

struct InlineView<'a> {
    elf: &'a Path,
    file: &'a object::File<'a>,
    head: &'a str,
    labels: Labels,
    at: &'a str,
    target: &'a inline::Target<'a>,
    out: Vec<String>,
    warnings: &'a mut Vec<String>,
}

impl InlineView<'_> {
    fn header(&mut self, k: usize) {
        let t = self.target;
        let hit = &t.instances[k];
        let sym = &t.symbols[hit.sym];
        let site = hit.inst.call.as_ref().map_or("?".to_string(), ToString::to_string);
        let base = format!("{} ({} of {}) in ", self.head, k + 1, t.instances.len());
        let place = format!(
            "[sym {} of {}] {} ({} / {}) at {site}",
            hit.sym + 1,
            t.symbols.len(),
            inline::demangled(&sym.name),
            hit.j + 1,
            sym.instances.len()
        );
        let inside = symbol_range(self.file, &sym.name)
            .is_some_and(|(lo, hi)| hit.inst.ranges.iter().all(|(a, b)| *a >= lo && *b <= hi));
        if inside {
            self.out.push(format!("{base}{place}"));
        } else {
            self.warnings.push(format!(
                "{}: instance {} of '{}' is not inside the ELF symbol '{}'",
                self.at,
                k + 1,
                t.name,
                inline::demangled(&sym.name)
            ));
            self.out.push(format!("{base}**{place}** WARN: Symbol name provided does not inline this"));
        }
    }

    fn disassemble(&mut self, ranges: &[(u64, u64)], marks: &[(u64, u64)], labels_end: Option<u64>) -> Result<()> {
        self.out.extend(disassemble_ranges(self.elf, self.file, ranges, self.labels, marks, labels_end)?);
        Ok(())
    }

    fn windows(&self, symbol: (u64, u64), marks: &[(u64, u64)], context: inline::Context) -> Result<Vec<(u64, u64)>> {
        let addrs: Vec<u64> = objdump_range(self.elf, symbol.0, symbol.1)?.iter().filter_map(|l| insn_addr(l)).collect();
        let mut keep = vec![false; addrs.len()];
        for (i, a) in addrs.iter().enumerate() {
            if in_ranges(marks, *a) {
                keep[i.saturating_sub(context.up)..=(i + context.down).min(addrs.len() - 1)].fill(true);
            }
        }
        let mut out: Vec<(u64, u64)> = Vec::new();
        for (i, a) in addrs.iter().enumerate().filter(|(i, _)| keep[*i]) {
            let end = addrs.get(i + 1).copied().unwrap_or(symbol.1);
            match out.last_mut() {
                Some((_, hi)) if *hi == *a => *hi = end,
                _ => out.push((*a, end)),
            }
        }
        Ok(out)
    }

    fn marked(&mut self, symbol: Option<(u64, u64)>, marks: &[(u64, u64)], context: Option<inline::Context>) -> Result<()> {
        match (symbol, context) {
            (Some(range), Some(context)) => {
                let windows = self.windows(range, marks, context)?;
                let tail = windows.last().is_some_and(|w| w.1 == range.1);
                self.disassemble(&windows, marks, tail.then_some(range.1))
            }
            (Some(range), None) => self.disassemble(&[range], marks, Some(range.1)),
            (None, _) => self.disassemble(marks, &[], None),
        }
    }

    fn render(&mut self, pick: inline::Pick, item: &inline::Item) -> Result<()> {
        let t = self.target;
        let text = item.text.as_str();
        if !self.out.is_empty() {
            self.out.push(String::new());
        }
        match pick {
            inline::Pick::Instance(k) => {
                self.header(k);
                let hit = &t.instances[k];
                match item.context {
                    Some(_) => self.marked(symbol_range(self.file, &t.symbols[hit.sym].name), &hit.inst.ranges, item.context)?,
                    None => self.disassemble(&hit.inst.ranges, &[], None)?,
                }
            }
            inline::Pick::Whole(i) => {
                let sym = &t.symbols[i];
                for &k in &sym.instances {
                    self.header(k);
                }
                let marks: Vec<(u64, u64)> = sym.instances.iter().flat_map(|&k| t.instances[k].inst.ranges.iter().copied()).collect();
                let range = symbol_range(self.file, &sym.name);
                if range.is_none() {
                    self.out.push(format!("-- '{}' is not an ELF symbol; showing the inlined ranges only", inline::demangled(&sym.name)));
                }
                self.marked(range, &marks, item.context)?;
            }
            inline::Pick::Absent { whole } => {
                let sel = Cpu::split(text).1;
                self.out.push(format!("{} in **{sel}** WARN: Symbol name provided does not inline this", self.head));
                self.warnings.push(format!("{}: '{sel}' does not inline '{}' (it is inlined in: {})", self.at, t.name, t.symbol_list()));
                if whole
                    && item.context.is_none()
                    && let Some(range) = symbol_range(self.file, sel)
                {
                    self.disassemble(&[range], &[], Some(range.1))?;
                }
            }
            inline::Pick::Nothing(why) => {
                self.out.push(format!("-- {text}: selects nothing; {why}"));
                self.warnings.push(format!("{}: {text} selects nothing ({why})", self.at));
            }
        }
        Ok(())
    }
}

fn render_label(line: &str, pad: usize, width: usize, label_first: bool) -> Option<Vec<String>> {
    let (id, comment) = line.strip_prefix(MARK_TAG)?.split_once('\t')?;
    let head = if label_first && !id.is_empty() { format!("({id}) \"") } else { "\"".to_string() };
    let tail = if !label_first && !id.is_empty() { format!("\"  ({id})") } else { "\"".to_string() };
    let first_col = pad.saturating_sub(2);
    let avail = width.saturating_sub(first_col + head.len()).max(20);
    let mut rows: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in comment.split(' ').filter(|w| !w.is_empty()) {
        if !cur.is_empty() && cur.len() + 1 + word.len() > avail {
            rows.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    rows.push(cur);
    let last = rows.len() - 1;
    Some(
        rows.into_iter()
            .enumerate()
            .map(|(i, r)| {
                let lead = if i == 0 { format!("{}{head}", " ".repeat(first_col)) } else { " ".repeat(pad) };
                let end = if i == last { tail.as_str() } else { "" };
                defuse(&format!("{lead}{r}{end}"))
            })
            .collect(),
    )
}

fn annotate_loads(lines: Vec<String>, resolve: impl Fn(u64) -> Option<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| {
            let resolved = WORD_LOAD
                .captures(&line)
                .and_then(|c| u64::from_str_radix(&c[1], 16).ok())
                .and_then(&resolve);
            match resolved {
                Some(r) => format!("{line}\t=> {r}"),
                None => line,
            }
        })
        .collect()
}

fn annotate_corpus_listing(lines: &[String]) -> Vec<String> {
    let words: HashMap<u64, u32> = lines
        .iter()
        .filter_map(|l| INSN.captures(l))
        .filter_map(|c| {
            let addr = u64::from_str_radix(c[1].trim().trim_end_matches(':'), 16).ok()?;
            let value = u32::from_str_radix(&WORD.captures(&c[3])?[1], 16).ok()?;
            Some((addr, value))
        })
        .collect();
    annotate_loads(lines.to_vec(), |addr| words.get(&addr).map(|v| format!("{v:#010x}")))
}

fn expand_tabs(s: &str) -> String {
    let mut out = String::new();
    let mut col = 0;
    for ch in s.chars() {
        if ch == '\t' {
            let n = 8 - col % 8;
            out.extend(std::iter::repeat_n(' ', n));
            col += n;
        } else {
            out.push(ch);
            col += 1;
        }
    }
    out
}

fn defuse(line: &str) -> String {
    line.replace("/*", "/ *").replace("*/", "* /")
}

fn normalize(line: &str, encodings: bool) -> String {
    let line = match INSN.captures(line) {
        Some(c) if !encodings => format!("{}\t{}", &c[1], &c[3]),
        _ => line.to_string(),
    };
    defuse(&expand_tabs(&line))
}

struct Insn<'a> {
    addr: &'a str,
    enc: &'a str,
    mnem: &'a str,
    ops: Option<&'a str>,
    comment: Option<&'a str>,
    resolved: Option<&'a str>,
}

fn parse_insn(line: &str) -> Option<Insn<'_>> {
    let c = INSN.captures(line)?;
    let mut fields = c.get(3)?.as_str().split('\t');
    let mut insn = Insn {
        addr: c.get(1)?.as_str().trim(),
        enc: c.get(2)?.as_str().trim(),
        mnem: fields.next()?,
        ops: None,
        comment: None,
        resolved: None,
    };
    for f in fields {
        if f.starts_with("@ ") {
            insn.comment = Some(f);
        } else if f.starts_with("=> ") {
            insn.resolved = Some(f);
        } else if insn.ops.is_none() && insn.comment.is_none() {
            insn.ops = Some(f);
        } else {
            return None;
        }
    }
    Some(insn)
}

fn fold_resolved(resolved: &str, pad: usize, max_width: usize) -> Vec<String> {
    let Some((head, names)) = resolved.split_once(" <").and_then(|(h, n)| Some((h, n.strip_suffix('>')?))) else {
        return vec![format!("{}{resolved}", " ".repeat(pad))];
    };
    let first = format!("{}{head} <", " ".repeat(pad));
    let cont = " ".repeat(first.len());
    let mut out = Vec::new();
    let mut cur = first;
    let mut fresh = true;
    let names: Vec<&str> = names.split(", ").collect();
    for (i, name) in names.iter().enumerate() {
        let tail = if i + 1 == names.len() { ">" } else { "," };
        let piece = format!("{}{name}{tail}", if fresh { "" } else { " " });
        if !fresh && cur.len() + piece.len() > max_width {
            out.push(cur);
            cur = format!("{cont}{name}{tail}");
        } else {
            cur.push_str(&piece);
        }
        fresh = false;
    }
    out.push(cur);
    out
}

fn compact_units(sides: [&[String]; 2], encodings: bool, label_first: bool) -> [Vec<Vec<String>>; 2] {
    let insns: Vec<Insn> = sides.iter().flat_map(|s| s.iter()).filter_map(|l| parse_insn(l)).collect();
    let aw = insns.iter().map(|i| i.addr.len()).max().unwrap_or(0);
    let ew = if encodings { insns.iter().map(|i| i.enc.len()).max().unwrap_or(0) + 1 } else { 0 };
    let mw = insns.iter().map(|i| i.mnem.len()).max().unwrap_or(0);
    let opcol = aw + 1 + ew + mw + 1;
    let inline_comment = |c: &str| !c.contains('<');
    let first_line = |i: &Insn| {
        let enc = if encodings { format!("{:<w$}", i.enc, w = ew) } else { String::new() };
        let mut l = format!("{:>aw$} {enc}{:<mw$} {}", i.addr, i.mnem, i.ops.unwrap_or(""));
        if let Some(c) = i.comment.filter(|c| inline_comment(c)) {
            l.push_str("  ");
            l.push_str(c);
        }
        l.trim_end().to_string()
    };
    let max_width = insns.iter().map(|i| first_line(i).len()).max().unwrap_or(0).max(opcol + 24);
    sides.map(|side| {
        side.iter()
            .map(|line| {
                if let Some(rows) = render_label(line, aw + 1, max_width.max(56), label_first) {
                    return rows;
                }
                let Some(i) = parse_insn(line) else {
                    return vec![defuse(&expand_tabs(line))];
                };
                let mut out = vec![defuse(&first_line(&i))];
                if let Some(c) = i.comment.filter(|c| !inline_comment(c)) {
                    out.push(defuse(&format!("{}{c}", " ".repeat(opcol + 5))));
                }
                if let Some(r) = i.resolved {
                    out.extend(fold_resolved(r, opcol, max_width).iter().map(|l| defuse(l)));
                }
                out
            })
            .collect()
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpClass {
    Branch,
    Load,
    Store,
    Alu,
    Compare,
    Other,
}

enum Key {
    Insn { base: String, cond: String, class: OpClass, konst: Option<String>, ops: String },
    Word,
    FuncLabel,
    Header,
    Ellipsis,
    Unmatched,
}

fn split_mnemonic(m: &str) -> (String, String, OpClass) {
    let groups: [(&[&str], OpClass); 6] = [
        (&BRANCH_OPS, OpClass::Branch),
        (&LOAD_OPS, OpClass::Load),
        (&STORE_OPS, OpClass::Store),
        (&ALU_OPS, OpClass::Alu),
        (&COMPARE_OPS, OpClass::Compare),
        (&OTHER_OPS, OpClass::Other),
    ];
    let mut best: Option<(&str, &str, OpClass)> = None;
    for (ops, class) in groups {
        for op in ops {
            let Some(rest) = m.strip_prefix(op) else { continue };
            let rest_cond = rest.strip_prefix('s').filter(|_| class == OpClass::Alu).unwrap_or(rest);
            let rest_cond = if class == OpClass::Alu { rest_cond.strip_suffix('s').unwrap_or(rest_cond) } else { rest_cond };
            if (rest_cond.is_empty() || CONDS.contains(&rest_cond)) && best.is_none_or(|(b, _, _)| op.len() > b.len()) {
                best = Some((op, rest_cond, class));
            }
        }
    }
    match best {
        Some((b, c, class)) => (b.to_string(), c.to_string(), class),
        None => (m.to_string(), String::new(), OpClass::Other),
    }
}

fn key_of(line: &str) -> Key {
    if line.starts_with(MARK_TAG) {
        return Key::Unmatched;
    }
    if let Some(i) = parse_insn(line) {
        if i.mnem == ".word" {
            return Key::Word;
        }
        let (base, cond, class) = split_mnemonic(i.mnem);
        let ops = i.ops.unwrap_or("");
        let (dest, src) = ops.split_once(", ").unwrap_or((ops, ""));
        let materializes = match base.as_str() {
            "mov" | "mvn" => src.starts_with('#'),
            "ldr" => src.starts_with("[pc"),
            _ => false,
        };
        let konst = materializes.then(|| dest.to_string());
        let ops = if ops.contains("[pc") || ops.contains('<') { String::new() } else { ops.to_string() };
        return Key::Insn { base, cond, class, konst, ops };
    }
    if LABEL.is_match(line) {
        Key::FuncLabel
    } else if line.starts_with("== ") {
        Key::Header
    } else if line == "..." {
        Key::Ellipsis
    } else {
        Key::Unmatched
    }
}

fn pair_score(a: &Key, b: &Key) -> Option<u32> {
    match (a, b) {
        (
            Key::Insn { base: ba, cond: ca, class: ka, konst: xa, ops: oa },
            Key::Insn { base: bb, cond: cb, class: kb, konst: xb, ops: ob },
        ) => {
            if xa.is_some() != xb.is_some() {
                (ka == kb).then_some(1)
            } else if ba == bb && ca == cb {
                Some(if !oa.is_empty() && oa == ob { 5 } else { 4 })
            } else if ba == bb {
                Some(if *ka == OpClass::Branch { 3 } else { 2 })
            } else if let (Some(da), Some(db)) = (xa, xb) {
                Some(if da == db { 3 } else { 2 })
            } else if ka == kb && *ka != OpClass::Other {
                Some(1)
            } else {
                None
            }
        }
        (Key::Word, Key::Word) => Some(2),
        (Key::FuncLabel, Key::FuncLabel) | (Key::Header, Key::Header) => Some(6),
        (Key::Ellipsis, Key::Ellipsis) => Some(1),
        _ => None,
    }
}

fn align_units(left: &[Key], right: &[Key]) -> Option<Vec<(Option<usize>, Option<usize>)>> {
    let (n, m) = (left.len(), right.len());
    if (n + 1) * (m + 1) > ALIGN_MAX_CELLS {
        return None;
    }
    let w = m + 1;
    let at = |i: usize, j: usize| i * w + j;
    let mut best = [vec![0i32; (n + 1) * w], vec![0i32; (n + 1) * w], vec![0i32; (n + 1) * w]];
    let options = |best: &[Vec<i32>; 3], i: usize, j: usize, state: usize| {
        let pair = (i < n && j < m)
            .then(|| pair_score(&left[i], &right[j]))
            .flatten()
            .map(|p| p as i32 + best[0][at(i + 1, j + 1)]);
        let del = (i < n).then(|| best[1][at(i + 1, j)] - GAP_EXTEND - if state == 1 { 0 } else { GAP_OPEN });
        let ins = (j < m).then(|| best[2][at(i, j + 1)] - GAP_EXTEND - if state == 2 { 0 } else { GAP_OPEN });
        [pair, del, ins]
    };
    for i in (0..=n).rev() {
        for j in (0..=m).rev() {
            if i == n && j == m {
                continue;
            }
            for state in 0..3 {
                let v = options(&best, i, j, state).into_iter().flatten().max().unwrap();
                best[state][at(i, j)] = v;
            }
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j, mut state) = (0, 0, 0);
    while i < n || j < m {
        let target = best[state][at(i, j)];
        let [pair, del, _] = options(&best, i, j, state);
        if pair == Some(target) {
            out.push((Some(i), Some(j)));
            (i, j, state) = (i + 1, j + 1, 0);
        } else if del == Some(target) {
            out.push((Some(i), None));
            (i, state) = (i + 1, 1);
        } else {
            out.push((None, Some(j)));
            (j, state) = (j + 1, 2);
        }
    }
    Some(out)
}

fn render_block(indent: &str, stage: &str, left: &[String], right: &[String], opts: Options) -> Vec<String> {
    let ltitle = format!("asm-corpus {stage}");
    let rtitle = format!("rsdec {stage}");
    let list = match opts.style {
        Style::List => true,
        Style::Columns => false,
        Style::Auto => left.len() <= LIST_STYLE_MAX_LINES && right.len() <= LIST_STYLE_MAX_LINES,
    };
    let keys = [left, right].map(|s| s.iter().map(|l| key_of(l)).collect::<Vec<_>>());
    let units = if opts.compact {
        compact_units([left, right], opts.encodings, opts.label_first)
    } else {
        let width = [left, right]
            .iter()
            .flat_map(|s| s.iter())
            .filter(|l| !l.starts_with(MARK_TAG))
            .map(|l| normalize(l, opts.encodings).len())
            .max()
            .unwrap_or(0)
            .max(56);
        [left, right].map(|s| {
            s.iter()
                .map(|l| render_label(l, 16, width, opts.label_first).unwrap_or_else(|| vec![normalize(l, opts.encodings)]))
                .collect()
        })
    };
    let alignment = if opts.align && !list { align_units(&keys[0], &keys[1]) } else { None };
    let [left, right] = match alignment {
        Some(steps) => {
            let (mut l, mut r) = (Vec::new(), Vec::new());
            let mut emit = |la: Vec<String>, rb: Vec<String>| {
                for k in 0..la.len().max(rb.len()) {
                    l.push(la.get(k).cloned().unwrap_or_default());
                    r.push(rb.get(k).cloned().unwrap_or_default());
                }
            };
            let (mut hunk_l, mut hunk_r) = (Vec::new(), Vec::new());
            for (a, b) in steps {
                match (a, b) {
                    (Some(i), Some(j)) => {
                        emit(std::mem::take(&mut hunk_l), std::mem::take(&mut hunk_r));
                        emit(units[0][i].clone(), units[1][j].clone());
                    }
                    (Some(i), None) => hunk_l.extend(units[0][i].iter().cloned()),
                    (None, Some(j)) => hunk_r.extend(units[1][j].iter().cloned()),
                    (None, None) => {}
                }
            }
            emit(hunk_l, hunk_r);
            [l, r]
        }
        None => units.map(|side| side.into_iter().flatten().collect::<Vec<String>>()),
    };
    let (left, right) = (left.as_slice(), right.as_slice());
    if list {
        let mut out = vec![format!("{indent}{BLOCK_START} {stage}")];
        for (title, lines) in [(&ltitle, left), (&rtitle, right)] {
            out.push(format!("{indent}{title}"));
            out.push(format!("{indent}{}", "-".repeat(title.len())));
            out.extend(lines.iter().map(|l| format!("{indent}{l}").trim_end().to_string()));
        }
        out.push(format!("{indent}{BLOCK_END}"));
        return out;
    }
    let width = |v: &[String], t: &str| v.iter().map(|l| l.chars().count()).chain([t.len()]).max().unwrap_or(0);
    let lw = width(left, &ltitle);
    let rw = width(right, &rtitle);
    let row = |l: &str, r: &str| format!("{indent}{l:<lw$} | {r}").trim_end().to_string();
    let mut out = vec![format!("{indent}{BLOCK_START} {stage}"), row(&ltitle, &rtitle)];
    out.push(format!("{indent}{}-+-{}", "-".repeat(lw), "-".repeat(rw)));
    for i in 0..left.len().max(right.len()) {
        out.push(row(left.get(i).map_or("", String::as_str), right.get(i).map_or("", String::as_str)));
    }
    out.push(format!("{indent}{BLOCK_END}"));
    out
}

fn pipeline(rsdec: &Path, corpus_root: &Path, mode: Mode, opts: Options, retry: bool) -> Result<()> {
    let lock = read_lock(rsdec)?;
    let corpus = walk_corpus(corpus_root)?;
    let lock_paths: BTreeSet<&str> = lock.iter().map(|e| e.path.as_str()).collect();
    let corpus_paths: BTreeSet<&str> = corpus.copies.keys().map(String::as_str).collect();
    if lock_paths != corpus_paths {
        let missing: Vec<&&str> = lock_paths.difference(&corpus_paths).take(5).collect();
        let extra: Vec<&&str> = corpus_paths.difference(&lock_paths).take(5).collect();
        bail!(
            "{LOCK_FILE} does not match the corpus (missing from corpus: {missing:?}, not in lock: {extra:?}); \
             regenerate it with `cargo xtask corpus lock --corpus <dir>`"
        );
    }

    let mut errors = Vec::new();
    let mut stubs = matches!(mode, Mode::Run | Mode::Stub).then(|| Stubs { lock: &lock, corpus: &corpus, mode: opts.stub_mode, seen: HashMap::new(), written: 0, warnings: Vec::new(), modules: HashMap::new() });
    if mode == Mode::Stub {
        let stubs = stubs.as_mut().unwrap();
        let files = source_files(rsdec)?;
        let texts: Vec<String> = files.iter().map(fs::read_to_string).collect::<std::io::Result<_>>()?;
        for (path, text) in files.iter().zip(&texts) {
            stubs.note_sections(&rel_str(rsdec, path), text);
        }
        let mut expanded = Vec::new();
        for (path, text) in files.iter().zip(&texts) {
            expanded.push(stubs.expand(&rel_str(rsdec, path), text, &mut errors)?);
        }
        if !errors.is_empty() {
            bail!("{}", errors.join("\n"));
        }
        for ((path, text), new) in files.iter().zip(&texts).zip(&expanded) {
            if new != text {
                write_if_unchanged(path, &rel_str(rsdec, path), text, new)?;
            }
        }
        for w in &stubs.warnings {
            warn(w);
        }
        println!("stubs written: {}", stubs.written);
        return Ok(());
    }
    let sources = load_sources(rsdec, &mut errors, stubs.as_mut())?;
    let stubs_written = stubs.as_ref().map_or(0, |s| s.written);
    let stub_warnings = stubs.as_ref().map_or(Vec::new(), |s| s.warnings.clone());

    let mut raw_cmds = Vec::new();
    for (si, s) in sources.iter().enumerate() {
        for c in &s.commands {
            let mut units = Vec::new();
            for (q, mods) in &c.units {
                match resolve_unit(&lock, q) {
                    Ok(i) => units.push((i, mods.clone())),
                    Err(e) => errors.push(format!("{}:{}: {e}", s.rel, c.line + 1)),
                }
            }
            raw_cmds.push((si, c, units));
        }
    }
    if !errors.is_empty() {
        bail!("{}", errors.join("\n"));
    }

    let mut records: HashMap<usize, Record> = HashMap::new();
    for (_, _, units) in &raw_cmds {
        for &(u, _) in units {
            if let std::collections::hash_map::Entry::Vacant(v) = records.entry(u) {
                let path = corpus.root.join(corpus.preferred_copy(&lock[u].path));
                let rec: Record =
                    serde_json::from_slice(&fs::read(&path)?).with_context(|| format!("parsing {}", path.display()))?;
                v.insert(rec);
            }
        }
    }

    let cmds: Vec<Cmd> = raw_cmds
        .into_iter()
        .map(|(si, c, units)| {
            let recs: Vec<&Record> = units.iter().map(|(u, _)| &records[u]).collect();
            let state = state_of(&recs, &c.done);
            Cmd {
                source: si,
                line: c.line,
                indent: c.indent.clone(),
                units,
                syms: c.syms.clone(),
                done: c.done.clone(),
                state,
            }
        })
        .collect();

    let mut target: BTreeMap<&str, &str> = BTreeMap::new();
    for c in &cmds {
        if let State::Complete(group) = &c.state {
            for &(u, _) in c.units.iter().filter(|(_, m)| !m.ignore) {
                let path = lock[u].path.as_str();
                if let Some(prev) = target.insert(path, group)
                    && prev != group
                {
                    errors.push(format!(
                        "{}:{}: {path} is DONE in both '{prev}' and '{group}'",
                        sources[c.source].rel,
                        c.line + 1
                    ));
                }
            }
        }
    }
    if !errors.is_empty() {
        bail!("{}", errors.join("\n"));
    }

    if mode == Mode::Status {
        print_status(&lock, &sources, &cmds, &target, opts.group_by_paths);
        return Ok(());
    }

    let mut new_texts: Vec<String> = sources.iter().map(|s| s.original.clone()).collect();
    let mut block_count = 0;
    let mut inline_warnings: Vec<String> = Vec::new();
    if mode == Mode::Run {
        let mut builds = Builds { rsdec: rsdec.to_path_buf(), indexes: HashMap::new(), elves: HashMap::new(), tables: HashMap::new() };
        let texts: HashMap<PathBuf, String> = sources.iter().map(|s| (s.path.clone(), s.stripped.clone())).collect();
        let mut normalized = 0;
        for src in &sources {
            if let Some(cpu) = Cpu::of_path(rsdec, &src.path)
                && let Some(fixed) = keep::normalize(&src.original, cpu)
                && write_if_unchanged(&src.path, &src.rel, &src.original, &fixed)?
            {
                normalized += 1;
            }
        }
        if normalized > 0 {
            println!("keep list: regenerated {normalized} section(s) in canonical form; running again");
            return pipeline(rsdec, corpus_root, mode, opts, retry);
        }
        let mut missing: BTreeMap<Cpu, BTreeSet<String>> = BTreeMap::new();
        let mut resolved: HashMap<(usize, usize), (Cpu, String)> = HashMap::new();
        for (ci, c) in cmds.iter().enumerate() {
            let State::Stage(stage) = c.state else { continue };
            for (qi, (spec, _)) in c.syms.iter().enumerate() {
                for q in spec.keep_queries() {
                    match builds.resolve_sym(&q, &texts)? {
                        Ok((cpu, sym)) => {
                            if !builds.table(cpu, stage)?.contains(&sym) {
                                missing.entry(cpu).or_default().insert(sym.clone());
                            }
                            if let SymSpec::Plain(_) = spec {
                                resolved.insert((ci, qi), (cpu, sym));
                            }
                        }
                        Err(e) => errors.push(format!("{}:{}: {e}", sources[c.source].rel, c.line + 1)),
                    }
                }
            }
        }
        if !missing.is_empty() && errors.is_empty() {
            if opts.auto_keep && !retry {
                let added = extend_keep_lists(rsdec, &sources, &missing)?;
                println!("keep list: added {added} symbol(s) the linker had removed; running again");
                return pipeline(rsdec, corpus_root, mode, opts, true);
            }
            for (cpu, syms) in &missing {
                for sym in syms {
                    errors.push(if opts.auto_keep {
                        format!("'{sym}' is still not in the {} ELF after adding it to the keep list", cpu.label())
                    } else {
                        format!(
                            "'{sym}' is not in the {} ELF (removed by --gc-sections); rerun without --no-auto-keep or add it with keep-symbols-list",
                            cpu.label()
                        )
                    });
                }
            }
        }
        if !errors.is_empty() {
            bail!("{}", errors.join("\n"));
        }
        let mut blocks: Vec<BTreeMap<usize, Vec<String>>> = vec![BTreeMap::new(); sources.len()];
        let mut dwarf: HashMap<(Cpu, Option<&'static str>), inline::Info> = HashMap::new();
        for (ci, c) in cmds.iter().enumerate() {
            let State::Stage(stage) = c.state else { continue };
            if c.units.is_empty() && c.syms.is_empty() {
                continue;
            }
            let at = format!("{}:{}", sources[c.source].rel, c.line + 1);
            let mut left = Vec::new();
            for (u, mods) in &c.units {
                let e = &lock[*u];
                let rec = &records[u];
                if !left.is_empty() {
                    left.push(String::new());
                }
                left.push(format!("== {}", e.path.trim_end_matches(".json")));
                left.push(format!("   {}", rec.source));
                left.extend(rec.notes.iter().map(|n| format!("!! {n}")));
                let listing = |r: &str| rec.regions.get(r).and_then(|x| x.listing.as_ref());
                match rec.regions.get(stage) {
                    Some(RegionEntry { listing: Some(l), .. }) => {
                        left.extend(mods.lines.apply(annotate_corpus_listing(l), opts.labels))
                    }
                    Some(RegionEntry { same_as: Some(b), .. }) => {
                        left.push(format!("-- {stage} bytes identical to {b}"));
                        let l = listing(b).map(|l| annotate_corpus_listing(l)).unwrap_or_default();
                        left.extend(mods.lines.apply(l, opts.labels));
                    }
                    _ => left.push(format!("-- absent in {stage}")),
                }
            }
            let mut right = Vec::new();
            for (qi, (spec, filter)) in c.syms.iter().enumerate() {
                if !right.is_empty() {
                    right.push(String::new());
                }
                let lines = match spec {
                    SymSpec::Plain(_) => {
                        let (cpu, sym) = &resolved[&(ci, qi)];
                        right.push(format!("== {}/{sym} ({})", cpu.label(), cpu.feature(stage).unwrap_or("default")));
                        disassemble(&builds.elf(*cpu, stage)?, sym, opts.labels)
                    }
                    SymSpec::Inline(spec) => inline_lines(&mut builds, &mut dwarf, spec, stage, opts.labels, &at, &mut inline_warnings),
                };
                match lines {
                    Ok(lines) => right.extend(filter.apply(lines, opts.labels)),
                    Err(e) => errors.push(format!("{at}: {e}")),
                }
            }
            blocks[c.source].entry(c.line).or_default().extend(render_block(&c.indent, stage, &left, &right, opts));
            block_count += 1;
        }
        if !errors.is_empty() {
            bail!("{}", errors.join("\n"));
        }
        for (i, s) in sources.iter().enumerate() {
            new_texts[i] = insert_blocks(&s.stripped, &blocks[i]);
        }
    }

    let mut ops = Vec::new();
    for (canon, copies) in &corpus.copies {
        let want = target.get(canon.as_str()).map_or(canon.clone(), |g| done_path(canon, g));
        if copies.len() == 1 && copies[0] == want {
            continue;
        }
        let src = if copies.contains(canon) {
            canon.clone()
        } else {
            let first = fs::read(corpus.root.join(&copies[0]))?;
            for other in &copies[1..] {
                if fs::read(corpus.root.join(other))? != first {
                    errors.push(format!("conflicting copies of {canon}: {}", copies.join(", ")));
                }
            }
            copies[0].clone()
        };
        if src != want {
            ops.push(Op::Move(src.clone(), want.clone()));
        }
        for c in copies {
            if *c != src && *c != want {
                ops.push(Op::Remove(c.clone()));
            }
        }
    }
    if !errors.is_empty() {
        bail!("{}", errors.join("\n"));
    }

    let mut rewritten = 0;
    for (s, new) in sources.iter().zip(&new_texts) {
        if *new != s.original && write_if_unchanged(&s.path, &s.rel, &s.original, new)? {
            rewritten += 1;
        }
    }

    let (mut moved, mut removed) = (0, 0);
    for op in &ops {
        match op {
            Op::Move(from, to) => {
                let to = corpus.root.join(to);
                fs::create_dir_all(to.parent().unwrap())?;
                fs::rename(corpus.root.join(from), &to)?;
                moved += 1;
            }
            Op::Remove(path) => {
                fs::remove_file(corpus.root.join(path))?;
                removed += 1;
            }
        }
    }
    prune_done_dirs(&corpus.root)?;

    let complete = cmds.iter().filter(|c| matches!(c.state, State::Complete(_))).count();
    println!(
        "commands: {} ({complete} complete), stubs: {stubs_written}, blocks: {block_count}, files rewritten: {rewritten}, corpus moves: {moved}, duplicates removed: {removed}",
        cmds.len()
    );
    for w in stub_warnings.iter().chain(&inline_warnings) {
        warn(w);
    }
    if mode == Mode::Run {
        for (s, text) in sources.iter().zip(&new_texts) {
            if COMMAND.is_match(text) {
                lint_text(&s.rel, text);
            }
        }
    }
    Ok(())
}

struct KeepEdits {
    texts: Vec<(PathBuf, String)>,
    by_path: HashMap<PathBuf, String>,
    rewritten: HashMap<PathBuf, String>,
}

impl KeepEdits {
    fn new(texts: Vec<(PathBuf, String)>) -> KeepEdits {
        let by_path = texts.iter().cloned().collect();
        KeepEdits { texts, by_path, rewritten: HashMap::new() }
    }

    fn current(&self, path: &Path) -> &str {
        self.rewritten.get(path).unwrap_or(&self.by_path[path])
    }

    fn update(&mut self, bin: &keep::Binary, f: impl FnOnce(BTreeSet<keep::Entry>) -> Option<BTreeSet<keep::Entry>>) -> Result<()> {
        let path = bin.require_keep_list(&self.texts)?;
        let text = self.current(&path);
        let new = keep::render(text, bin.cpu, f(keep::read(text)).as_ref());
        self.rewritten.insert(path, new);
        Ok(())
    }

    fn resolve_into(index: &keep::Index, symbol: &str, entries: &mut BTreeSet<keep::Entry>) -> Result<usize, String> {
        let resolved = index.resolve(symbol)?;
        if let Some(w) = resolved.warning {
            warn(&w);
        }
        Ok(resolved.entries.into_iter().map(|e| usize::from(entries.insert(e))).sum())
    }
}

fn extend_keep_lists(rsdec: &Path, sources: &[Source], missing: &BTreeMap<Cpu, BTreeSet<String>>) -> Result<usize> {
    let mut edits = KeepEdits::new(sources.iter().map(|s| (s.path.clone(), s.stripped.clone())).collect());
    let mut added = 0;
    let mut errors = Vec::new();
    for (cpu, syms) in missing {
        let bin = keep::Binary::load(rsdec, *cpu)?;
        let index = bin.index(&edits.by_path)?;
        edits.update(&bin, |mut entries| {
            for sym in syms {
                match KeepEdits::resolve_into(&index, sym, &mut entries) {
                    Ok(n) => added += n,
                    Err(e) => errors.push(format!("{}: {e}", cpu.label())),
                }
            }
            Some(entries)
        })?;
    }
    if !errors.is_empty() {
        bail!("cannot keep symbols the comparisons need:\n{}", errors.join("\n"));
    }
    for s in sources {
        let new = match edits.rewritten.get(&s.path) {
            Some(t) => t,
            None if s.stubbed => &s.stripped,
            None => continue,
        };
        if *new != s.original {
            write_if_unchanged(&s.path, &s.rel, &s.original, new)?;
        }
    }
    Ok(added)
}

fn keep_cmd(rsdec: &Path, requested: &[String], remove: bool) -> Result<()> {
    ensure!(!remove || requested.is_empty(), "--remove takes no symbols");
    let mut texts = Vec::new();
    for path in source_files(rsdec)? {
        let text = fs::read_to_string(&path)?;
        texts.push((path, text));
    }
    let mut edits = KeepEdits::new(texts);
    let bins: BTreeMap<Cpu, keep::Binary> = Cpu::ALL.into_iter().map(|c| Ok((c, keep::Binary::load(rsdec, c)?))).collect::<Result<_>>()?;
    let all = requested.is_empty();
    let mut per_cpu: BTreeMap<Cpu, BTreeSet<keep::Entry>> = BTreeMap::new();
    if !remove {
        let mut errors = Vec::new();
        let queries: BTreeSet<String> = if all {
            edits
                .texts
                .iter()
                .flat_map(|(path, text)| {
                    let stripped = strip_blocks(text).unwrap_or_else(|_| text.clone());
                    parse_commands(&rel_str(rsdec, path), &stripped, &mut errors).into_iter().flat_map(|c| c.syms).flat_map(|(s, _)| s.keep_queries())
                })
                .collect()
        } else {
            requested.iter().cloned().collect()
        };
        let indexes: BTreeMap<Cpu, keep::Index> = bins.iter().map(|(c, b)| Ok((*c, b.index(&edits.by_path)?))).collect::<Result<_>>()?;
        for q in &queries {
            let targets: Vec<(Cpu, &str)> = match Cpu::from_label(q) {
                Some((cpu, rest)) => vec![(cpu, rest)],
                None => indexes.iter().filter(|(_, ix)| ix.resolves(q)).map(|(cpu, _)| (*cpu, q.as_str())).collect(),
            };
            match targets.as_slice() {
                [(cpu, sym)] => {
                    if let Err(e) = KeepEdits::resolve_into(&indexes[cpu], sym, per_cpu.entry(*cpu).or_default()) {
                        errors.push(e);
                    }
                }
                [] => {
                    let why: Vec<String> =
                        indexes.iter().map(|(cpu, ix)| format!("{}: {}", cpu.label(), ix.resolve(q).err().unwrap_or_default())).collect();
                    errors.push(format!("'{q}' cannot be kept ({})", why.join(" | ")));
                }
                _ => errors.push(format!("'{q}' exists for both CPUs; qualify it as arm7_bin/{q} or arm9_bin/{q}")),
            }
        }
        if !errors.is_empty() {
            bail!("{}", errors.join("\n"));
        }
    }
    for (cpu, bin) in &bins {
        let entries = per_cpu.remove(cpu).unwrap_or_default();
        if !remove && !all && entries.is_empty() {
            continue;
        }
        if (remove || all) && bin.keep_list_file(&edits.texts)?.is_none() {
            if !remove && !entries.is_empty() {
                warn(&format!("{} has no {} directive; its {} entry(ies) were not listed", cpu.label(), keep::DIRECTIVE, entries.len()));
            }
            continue;
        }
        let count = entries.len();
        edits.update(bin, |_| (!remove).then_some(entries))?;
        if remove {
            println!("keep list for {}: removed", cpu.label());
        } else {
            println!("keep list for {}: {count} entry(ies)", cpu.label());
        }
    }
    for (path, new) in &edits.rewritten {
        let original = &edits.by_path[path];
        if new != original {
            write_if_unchanged(path, &rel_str(rsdec, path), original, new)?;
        }
    }
    Ok(())
}

fn prune_done_dirs(root: &Path) -> Result<()> {
    for family in ["arm7", "arm9"] {
        for module in fs::read_dir(root.join(family))? {
            let done = module?.path().join("DONE");
            if !done.is_dir() {
                continue;
            }
            for group in fs::read_dir(&done)? {
                let group = group?.path();
                if group.is_dir() && fs::read_dir(&group)?.next().is_none() {
                    fs::remove_dir(&group)?;
                }
            }
            if fs::read_dir(&done)?.next().is_none() {
                fs::remove_dir(&done)?;
            }
        }
    }
    Ok(())
}

fn print_status(lock: &[LockEntry], sources: &[Source], cmds: &[Cmd], target: &BTreeMap<&str, &str>, group_by_paths: bool) {
    let mut modules: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for e in lock {
        let m = modules.entry(e.dir()).or_default();
        m.1 += 1;
        if target.contains_key(e.path.as_str()) {
            m.0 += 1;
        }
    }
    let done = target.len();
    println!("done: {done} / {} units ({:.2}%)", lock.len(), 100.0 * done as f64 / lock.len() as f64);
    for (m, (d, t)) in &modules {
        if *d > 0 {
            println!("  {m:<12} {d} / {t}");
        }
    }
    let mut groups: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
    for (path, g) in target {
        let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
        *groups.entry(g).or_default().entry(dir).or_default() += 1;
    }
    if !groups.is_empty() {
        println!("groups:");
        let w = groups.keys().map(|g| g.len()).max().unwrap_or(0);
        for (g, dirs) in &groups {
            let n: usize = dirs.values().sum();
            let split = if group_by_paths && dirs.len() > 1 {
                format!(" ({})", dirs.iter().map(|(d, n)| format!("{d} => {n}")).collect::<Vec<_>>().join(", "))
            } else {
                String::new()
            };
            println!("  {g:<w$} {n}{split}");
        }
    }
    let pending: Vec<&Cmd> = cmds.iter().filter(|c| matches!(c.state, State::Stage(_))).collect();
    if !pending.is_empty() {
        println!("in progress:");
        for c in pending {
            let State::Stage(stage) = c.state else { continue };
            let done = c.done.as_ref().map_or(String::new(), |(g, r)| {
                format!(" DONE('{g}'{})", r.iter().map(|r| format!(", {r}")).collect::<String>())
            });
            let units: Vec<&str> = c.units.iter().map(|&(u, _)| lock[u].stem()).collect();
            println!(
                "  {}:{}  stage {stage}{done}  [{}]",
                sources[c.source].rel,
                c.line + 1,
                units.join(", ")
            );
        }
    }
}

fn source_files(rsdec: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    walk(rsdec, &SKIP_DIRS, &mut files)?;
    files.retain(|p| p.extension().is_some_and(|e| e == "rs" || e == "s"));
    files.sort();
    Ok(files)
}

fn write_if_unchanged(path: &Path, rel: &str, original: &str, new: &str) -> Result<bool> {
    if fs::read_to_string(path)? != original {
        warn(&format!("{rel}: changed on disk during this run; not rewritten"));
        return Ok(false);
    }
    let tmp = path.with_extension("xtask-tmp");
    fs::write(&tmp, new)?;
    fs::rename(&tmp, path)?;
    Ok(true)
}

fn strip_all(rsdec: &Path) -> Result<()> {
    let mut stripped = Vec::new();
    let mut errors = Vec::new();
    for path in source_files(rsdec)? {
        let rel = rel_str(rsdec, &path);
        let original = fs::read_to_string(&path)?;
        match strip_blocks(&original) {
            Ok(s) if s != original => stripped.push((path, rel, original, s)),
            Ok(_) => {}
            Err(line) => errors.push(format!("{rel}:{}: {BLOCK_START} without {BLOCK_END}", line + 1)),
        }
    }
    if !errors.is_empty() {
        bail!("{}", errors.join("\n"));
    }
    let mut n = 0;
    for (path, rel, original, s) in &stripped {
        if write_if_unchanged(path, rel, original, s)? {
            n += 1;
        }
    }
    println!("stripped blocks from {n} files");
    Ok(())
}
