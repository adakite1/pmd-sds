use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::LazyLock;

use regex::{Captures, Regex};

static REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<([^<>+\s]+)(\+0x[0-9a-f]+)?>").unwrap());
static VALUE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\t=> (0x([0-9a-f]{8}))$").unwrap());

const LEGEND_TAG: &str = "\x01legend\t";
const LEGEND_HEAD: usize = 3;
const LEGEND_TAIL: usize = 3;
const LEGEND_CONT: &str = "    ";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Legend {
    Short,
    Verbose,
}

pub fn is_legend(line: &str) -> bool {
    line.starts_with(LEGEND_TAG)
}

fn chunks(word: &str, room: usize) -> impl Iterator<Item = &str> {
    let mut rest = word;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let cut = rest.char_indices().nth(room).map_or(rest.len(), |(i, _)| i);
        let (head, tail) = rest.split_at(cut);
        rest = tail;
        Some(head)
    })
}

pub fn wrap_legend(line: &str, width: usize) -> Option<Vec<String>> {
    let text = line.strip_prefix(LEGEND_TAG)?;
    let width = width.max(LEGEND_CONT.len() + 8);
    let len = |s: &str| s.chars().count();
    let mut rows: Vec<String> = Vec::new();
    for word in text.split(' ').filter(|w| !w.is_empty()).flat_map(|w| chunks(w, width - LEGEND_CONT.len())) {
        match rows.last_mut() {
            Some(row) if len(row) + 1 + len(word) <= width => {
                row.push(' ');
                row.push_str(word);
            }
            Some(_) => rows.push(format!("{LEGEND_CONT}{word}")),
            None => rows.push(word.to_string()),
        }
    }
    Some(rows)
}

pub struct Unit {
    pub module: String,
    pub name: String,
    pub addrs: BTreeMap<String, u64>,
}

impl Unit {
    fn cpu(&self) -> &str {
        cpu_of(&self.module)
    }
}

struct Entry {
    unit: Unit,
    names: Vec<String>,
    done: bool,
}

impl Entry {
    fn counterpart(&self) -> String {
        let stars = if self.done { "*" } else { "**" };
        match self.names.as_slice() {
            [one] => format!("{stars}{one}"),
            many => format!("{stars}[{}]", many.join(", ")),
        }
    }
}

fn cpu_of(module: &str) -> &str {
    module.split('/').next().unwrap_or(module)
}

fn is_overlay(module: &str) -> bool {
    module.rsplit('/').next().is_some_and(|m| m.starts_with("ov"))
}

pub struct Renames {
    entries: BTreeMap<usize, Entry>,
    ambiguous: HashMap<String, HashSet<String>>,
    legend: Legend,
}

impl Renames {
    pub fn new<'a>(all: impl Iterator<Item = (&'a str, &'a str)>, legend: Legend) -> Renames {
        let mut modules: HashMap<(&str, &str), HashSet<&str>> = HashMap::new();
        for (module, name) in all {
            modules.entry((cpu_of(module), name)).or_default().insert(module);
        }
        let mut ambiguous: HashMap<String, HashSet<String>> = HashMap::new();
        for ((cpu, name), _) in modules.into_iter().filter(|(_, m)| m.len() > 1) {
            ambiguous.entry(cpu.to_string()).or_default().insert(name.to_string());
        }
        Renames { entries: BTreeMap::new(), ambiguous, legend }
    }

    pub fn add(&mut self, key: usize, unit: Unit, names: &[String], done: bool) {
        if names.is_empty() {
            return;
        }
        let e = self.entries.entry(key).or_insert(Entry { unit, names: Vec::new(), done: true });
        for n in names {
            if !e.names.contains(n) {
                e.names.push(n.clone());
            }
        }
        e.done &= done;
    }

    fn one<'a>(mut hits: impl Iterator<Item = &'a Entry>) -> Option<&'a Entry> {
        let first = hits.next()?;
        hits.next().is_none().then_some(first)
    }

    fn by_name(&self, module: &str, name: &str) -> Option<&Entry> {
        let cpu = cpu_of(module);
        let ambiguous = self.ambiguous.get(cpu).is_some_and(|names| names.contains(name));
        Renames::one(self.entries.values().filter(|e| {
            e.unit.cpu() == cpu && e.unit.name == name && (!ambiguous || e.unit.module == module)
        }))
    }

    fn by_value(&self, module: &str, region: &str, value: u64) -> Option<&Entry> {
        let cpu = cpu_of(module);
        Renames::one(self.entries.values().filter(|e| {
            e.unit.cpu() == cpu
                && e.unit.addrs.get(region) == Some(&(value & !1))
                && (!is_overlay(&e.unit.module) || e.unit.module == module)
        }))
    }

    fn legend(&self, renames: Vec<String>) -> Vec<String> {
        let n = renames.len();
        let shown = if self.legend == Legend::Verbose || n <= LEGEND_HEAD + LEGEND_TAIL + 1 {
            renames
        } else {
            let mut v = renames[..LEGEND_HEAD].to_vec();
            v.push(format!("... {} more renames here ...", n - LEGEND_HEAD - LEGEND_TAIL));
            v.extend_from_slice(&renames[n - LEGEND_TAIL..]);
            v
        };
        shown.into_iter().map(|l| format!("{LEGEND_TAG}{l}")).collect()
    }

    pub fn apply(&self, lines: Vec<String>, module: &str, region: &str) -> (Vec<String>, Vec<String>) {
        let mut renames: Vec<String> = Vec::new();
        let mut note = |l: String| {
            if !renames.contains(&l) {
                renames.push(l);
            }
        };
        let mut out = Vec::new();
        for line in lines {
            let mut line = REF
                .replace_all(&line, |c: &Captures| match self.by_name(module, &c[1]) {
                    Some(e) => {
                        note(format!("{} bulk renamed to {}", &c[1], e.counterpart()));
                        format!("<{}{}>", e.counterpart(), c.get(2).map_or("", |m| m.as_str()))
                    }
                    None => c[0].to_string(),
                })
                .into_owned();
            let hit = VALUE.captures(&line).and_then(|c| {
                let e = self.by_value(module, region, u64::from_str_radix(&c[2], 16).ok()?)?;
                Some((c[1].to_string(), e))
            });
            if let Some((value, e)) = hit {
                note(format!("{value} resolved as {} and then bulk renamed to {}", e.unit.name, e.counterpart()));
                line = format!("{line} <{}>", e.counterpart());
            }
            out.push(line);
        }
        (self.legend(renames), out)
    }
}
