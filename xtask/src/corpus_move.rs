use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use regex::Regex;

use crate::corpus_layout::{ItemSpan, item_spans, item_tokens};

static FROM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/\*\(move_from(?:\s*:\s*([A-Za-z_][A-Za-z0-9_]*))?\s*\)\*/\s*").unwrap());
static TO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/\*\(move\s*:\s*([A-Za-z_][A-Za-z0-9_]*)\s*,\s*([A-Za-z_]+)\s*\)\*/").unwrap());
const PREFIX: &str = "/*(move";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    No,
    IfClean,
    Strict,
}

struct File {
    path: PathBuf,
    rel: String,
    text: String,
    lines: Vec<String>,
}

impl File {
    fn at(&self, line: usize) -> String {
        format!("{}:{}", self.rel, line + 1)
    }

    fn blank(&self, line: usize) -> bool {
        self.lines.get(line).is_some_and(|l| l.trim().is_empty())
    }
}

struct Source {
    file: usize,
    marker: usize,
    id: String,
    lead: usize,
    first: usize,
    last: usize,
}

struct Target {
    file: usize,
    line: usize,
    id: String,
}

struct Move<'a> {
    src: &'a Source,
    dst: &'a Target,
    block: Vec<String>,
    blank_before: bool,
    blank_after: bool,
}

#[derive(Default)]
struct Errors(Vec<String>);

impl Errors {
    fn push(&mut self, msg: String) {
        self.0.push(msg);
    }

    fn check(self) -> Result<()> {
        if self.0.is_empty() { Ok(()) } else { bail!("{}", self.0.join("\n")) }
    }
}

fn indent_of(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

fn without_markers(line: &str) -> String {
    let s = FROM.replace_all(line, "");
    TO.replace_all(&s, "").into_owned()
}

fn non_blank(text: &str) -> Vec<&str> {
    text.split('\n').filter(|l| !l.trim().is_empty()).collect()
}

fn multiset(items: impl IntoIterator<Item = String>) -> BTreeMap<String, i64> {
    let mut m = BTreeMap::new();
    for s in items {
        *m.entry(s).or_insert(0) += 1;
    }
    m
}

fn load(rsdec: &Path, sources: &[PathBuf]) -> Result<Vec<File>> {
    let mut files = Vec::new();
    for path in sources.iter().filter(|p| p.extension().is_some_and(|e| e == "rs")) {
        let text = fs::read_to_string(path)?;
        if text.contains(PREFIX) {
            let rel = path.strip_prefix(rsdec).unwrap_or(path).to_string_lossy().replace('\\', "/");
            let lines = text.split('\n').map(String::from).collect();
            files.push(File { path: path.clone(), rel, text, lines });
        }
    }
    Ok(files)
}

fn owner<'a>(spans: &'a [ItemSpan], line: usize) -> Option<&'a ItemSpan> {
    spans.iter().filter(|s| s.lead <= line && line <= s.first).min_by_key(|s| s.last - s.lead)
}

fn shares_a_line(spans: &[ItemSpan], s: &ItemSpan) -> bool {
    let nested = |a: &ItemSpan, b: &ItemSpan| a.lead <= b.lead && b.last <= a.last;
    spans.iter().any(|o| !std::ptr::eq(o, s) && !nested(o, s) && !nested(s, o) && o.lead <= s.last && s.lead <= o.last)
}

fn scan(files: &[File]) -> Result<(Vec<Source>, Vec<Target>)> {
    let mut errors = Errors::default();
    let (mut froms, mut tos) = (Vec::new(), Vec::new());
    for (fi, f) in files.iter().enumerate() {
        let spans = item_spans(&f.text).map_err(|(l, m)| errors.push(format!("{}: parse failed ({m}); cannot move from or into it", f.at(l)))).ok();
        for (li, line) in f.lines.iter().enumerate() {
            let mut matched = BTreeSet::new();
            for c in FROM.captures_iter(line) {
                matched.insert(c.get(0).unwrap().start());
                let Some(spans) = &spans else { continue };
                let Some(s) = owner(spans, li) else {
                    errors.push(format!("{}: move_from is not attached to an item (put it in the comments directly above it or on its first line)", f.at(li)));
                    continue;
                };
                if shares_a_line(spans, s) {
                    errors.push(format!("{}: the item shares a line with a neighbouring item, split them first", f.at(li)));
                    continue;
                }
                let Some(id) = c.get(1).map(|m| m.as_str().to_string()).or_else(|| s.ident.clone()) else {
                    errors.push(format!("{}: the item has no name; give the marker an id with (/*(move_from: some_id)*/)", f.at(li)));
                    continue;
                };
                froms.push(Source { file: fi, marker: li, id, lead: s.lead, first: s.first, last: s.last });
            }
            for c in TO.captures_iter(line) {
                matched.insert(c.get(0).unwrap().start());
                if &c[2] != "here" {
                    errors.push(format!("{}: unknown move position '{}' (expected here)", f.at(li), &c[2]));
                } else if line.trim() != c.get(0).unwrap().as_str() {
                    errors.push(format!("{}: a move target must be alone on its line", f.at(li)));
                } else {
                    tos.push(Target { file: fi, line: li, id: c[1].to_string() });
                }
            }
            if line.match_indices(PREFIX).any(|(b, _)| !matched.contains(&b)) {
                errors.push(format!("{}: malformed move marker (expected /*(move_from)*/, /*(move_from: id)*/ or /*(move: id, here)*/)", f.at(li)));
            }
        }
    }
    errors.check()?;
    Ok((froms, tos))
}

fn pair<'a>(files: &[File], froms: &'a [Source], tos: &'a [Target]) -> Result<Vec<Move<'a>>> {
    let mut errors = Errors::default();
    let mut src_by_id: BTreeMap<&str, Vec<&Source>> = BTreeMap::new();
    let mut dst_by_id: BTreeMap<&str, Vec<&Target>> = BTreeMap::new();
    for s in froms {
        src_by_id.entry(&s.id).or_default().push(s);
    }
    for t in tos {
        dst_by_id.entry(&t.id).or_default().push(t);
    }
    for (id, ss) in &src_by_id {
        if ss.len() > 1 {
            let at: Vec<String> = ss.iter().map(|s| files[s.file].at(s.marker)).collect();
            errors.push(format!("move id '{id}' is used by several move_from markers ({}); give them explicit ids", at.join(", ")));
        }
        if !dst_by_id.contains_key(id) {
            errors.push(format!("{}: move_from '{id}' has no /*(move: {id}, here)*/", files[ss[0].file].at(ss[0].marker)));
        }
    }
    for (id, ts) in &dst_by_id {
        if ts.len() > 1 {
            let at: Vec<String> = ts.iter().map(|t| files[t.file].at(t.line)).collect();
            errors.push(format!("move id '{id}' has several targets ({})", at.join(", ")));
        }
        if !src_by_id.contains_key(id) {
            errors.push(format!("{}: move target '{id}' has no matching move_from", files[ts[0].file].at(ts[0].line)));
        }
    }
    for (i, a) in froms.iter().enumerate() {
        if froms[i + 1..].iter().any(|b| a.file == b.file && a.lead <= b.last && b.lead <= a.last) {
            errors.push(format!("{}: moved items overlap", files[a.file].at(a.marker)));
        }
        for t in tos.iter().filter(|t| t.file == a.file && (a.lead..=a.last).contains(&t.line)) {
            errors.push(format!("{}: move target is inside an item being moved", files[t.file].at(t.line)));
        }
    }
    errors.check()?;

    Ok(froms
        .iter()
        .map(|src| {
            let dst = dst_by_id[src.id.as_str()][0];
            let (f, to) = (&files[src.file], &files[dst.file]);
            let (from_indent, to_indent) = (indent_of(&f.lines[src.first]), indent_of(&to.lines[dst.line]));
            let block = f.lines[src.lead..=src.last]
                .iter()
                .filter_map(|line| {
                    let line = if FROM.is_match(line) {
                        let rest = FROM.replace_all(line, "");
                        if rest.trim().is_empty() {
                            return None;
                        }
                        format!("{}{}", indent_of(line), rest.trim_start())
                    } else {
                        line.clone()
                    };
                    Some(match line.strip_prefix(from_indent) {
                        Some(rest) if !line.trim().is_empty() => format!("{to_indent}{rest}"),
                        _ => line,
                    })
                })
                .collect();
            Move { src, dst, block, blank_before: src.lead > 0 && f.blank(src.lead - 1), blank_after: f.blank(src.last + 1) }
        })
        .collect())
}

fn apply(files: &[File], moves: &[Move]) -> Vec<String> {
    files
        .iter()
        .enumerate()
        .map(|(fi, f)| {
            let mut removed = vec![false; f.lines.len()];
            for m in moves.iter().filter(|m| m.src.file == fi) {
                let mut end = m.src.last;
                if m.blank_before && m.blank_after {
                    end += 1;
                }
                removed[m.src.lead..=end].fill(true);
            }
            let inserts: HashMap<usize, &Move> = moves.iter().filter(|m| m.dst.file == fi).map(|m| (m.dst.line, m)).collect();
            let mut out: Vec<String> = Vec::new();
            let mut blank_owed = false;
            let ends_blank = |out: &[String]| out.last().is_none_or(|l| l.trim().is_empty());
            for (li, line) in f.lines.iter().enumerate() {
                if let Some(m) = inserts.get(&li) {
                    if m.blank_before && !ends_blank(&out) {
                        out.push(String::new());
                    }
                    out.extend(m.block.iter().cloned());
                    blank_owed = m.blank_after;
                } else if !removed[li] {
                    if blank_owed && !line.trim().is_empty() {
                        out.push(String::new());
                    }
                    blank_owed = false;
                    out.push(line.clone());
                }
            }
            out.join("\n")
        })
        .collect()
}

fn verify(files: &[File], new_texts: &[String]) -> Result<()> {
    let mut problems = Vec::new();
    let (mut before, mut after) = (Vec::new(), Vec::new());
    for (f, new) in files.iter().zip(new_texts) {
        before.extend(item_tokens(&f.text).map_err(|(l, m)| anyhow::anyhow!("{}: {m}", f.at(l)))?);
        match item_tokens(new) {
            Ok(t) => after.extend(t),
            Err((l, m)) => problems.push(format!("{} would not parse after the move (line {}: {m})", f.rel, l + 1)),
        }
    }
    if problems.is_empty() {
        problems.extend(compare("items (as tokens)", multiset(before), multiset(after)));
    }
    let lines_before = files.iter().flat_map(|f| f.lines.iter().map(|l| without_markers(l).trim().to_string()));
    let lines_after = new_texts.iter().flat_map(|t| t.split('\n').map(|l| l.trim().to_string()));
    problems.extend(compare(
        "non-blank lines",
        multiset(lines_before.filter(|l| !l.is_empty())),
        multiset(lines_after.filter(|l| !l.is_empty())),
    ));
    if !problems.is_empty() {
        bail!("move not applied; verification failed:\n{}", problems.join("\n"));
    }
    Ok(())
}

fn compare(what: &str, before: BTreeMap<String, i64>, after: BTreeMap<String, i64>) -> Option<String> {
    let keys: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    let changed: Vec<String> = keys
        .into_iter()
        .filter_map(|k| {
            let (b, a) = (before.get(k).copied().unwrap_or(0), after.get(k).copied().unwrap_or(0));
            (b != a).then(|| format!("  {b} -> {a}: {}", k.chars().take(120).collect::<String>()))
        })
        .collect();
    (!changed.is_empty()).then(|| format!("{what} changed:\n{}", changed[..changed.len().min(5)].join("\n")))
}

fn unstaged_changes(rsdec: &Path, f: &File) -> Result<Option<String>> {
    let out = Command::new("git").arg("-C").arg(rsdec).args(["show", &format!(":{}", f.rel)]).output().context("running git show")?;
    if !out.status.success() {
        return Ok(Some(format!("{}: not in the git index (untracked?)", f.rel)));
    }
    let index = String::from_utf8(out.stdout)?;
    let work: String = f.lines.iter().map(|l| without_markers(l)).collect::<Vec<_>>().join("\n");
    let (w, i) = (non_blank(&work), non_blank(&index));
    if w == i {
        return Ok(None);
    }
    let at = w.iter().zip(&i).position(|(a, b)| a != b).unwrap_or(w.len().min(i.len()));
    let shown = w.get(at).or(i.get(at)).map_or("", |s| s.trim());
    Ok(Some(format!("{}: differs from the index at its {}th non-blank line: {shown}", f.rel, at + 1)))
}

pub fn run(rsdec: &Path, sources: &[PathBuf], stage: Stage) -> Result<()> {
    let files = load(rsdec, sources)?;
    if files.is_empty() {
        println!("move: no move markers found");
        return Ok(());
    }
    let (froms, tos) = scan(&files)?;
    let moves = pair(&files, &froms, &tos)?;
    let new_texts = apply(&files, &moves);
    verify(&files, &new_texts)?;

    let touched: Vec<usize> = (0..files.len()).filter(|i| new_texts[*i] != files[*i].text).collect();
    let mut dirty: BTreeMap<usize, String> = BTreeMap::new();
    if stage != Stage::No {
        for &fi in &touched {
            if let Some(why) = unstaged_changes(rsdec, &files[fi])? {
                dirty.insert(fi, why);
            }
        }
    }
    if stage == Stage::Strict && !dirty.is_empty() {
        bail!(
            "refused due to --stage-strict (nothing written): these files have changes besides the move markers that are not staged:\n  {}",
            dirty.values().cloned().collect::<Vec<_>>().join("\n  ")
        );
    }

    for &fi in &touched {
        let f = &files[fi];
        if fs::read_to_string(&f.path)? != f.text {
            bail!("{} changed on disk during the move; stopped before writing it", f.rel);
        }
        let tmp = f.path.with_extension("xtask-tmp");
        fs::write(&tmp, &new_texts[fi])?;
        fs::rename(&tmp, &f.path)?;
    }
    for m in &moves {
        let (f, t) = (&files[m.src.file], &files[m.dst.file]);
        println!("moved {}: {}-{} -> {}", m.src.id, f.at(m.src.lead), m.src.last + 1, t.at(m.dst.line));
    }
    println!("move: {} item(s), {} file(s) rewritten, verified (items and non-blank lines unchanged)", moves.len(), touched.len());

    if stage != Stage::No {
        let blocked = blocked_by_dirty(&touched, &moves, &dirty);
        let rels: Vec<&str> = touched.iter().filter(|i| !blocked.contains(i)).map(|i| files[*i].rel.as_str()).collect();
        if !rels.is_empty() {
            if !Command::new("git").arg("-C").arg(rsdec).arg("add").arg("--").args(&rels).status()?.success() {
                bail!("git add failed");
            }
            println!("staged: {}", rels.join(", "));
        }
        if !blocked.is_empty() {
            let mut why: Vec<String> = dirty.values().cloned().collect();
            why.extend(blocked.iter().filter(|i| !dirty.contains_key(i)).map(|i| format!("{}: linked by a move to a file above", files[*i].rel)));
            crate::corpus::warn(&format!(
                "moved but not staged (staging would include changes that are not part of the move):\n  {}",
                why.join("\n  ")
            ));
        }
    }
    Ok(())
}

fn blocked_by_dirty(touched: &[usize], moves: &[Move], dirty: &BTreeMap<usize, String>) -> BTreeSet<usize> {
    let mut blocked: BTreeSet<usize> = dirty.keys().copied().collect();
    loop {
        let before = blocked.len();
        for m in moves {
            let (a, b) = (m.src.file, m.dst.file);
            if blocked.contains(&a) || blocked.contains(&b) {
                blocked.extend([a, b]);
            }
        }
        if blocked.len() == before {
            return blocked.into_iter().filter(|i| touched.contains(i)).collect();
        }
    }
}
