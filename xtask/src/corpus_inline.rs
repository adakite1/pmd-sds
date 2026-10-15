use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::ops::Range;
use std::sync::LazyLock;

use anyhow::{Context as _, Result};
use gimli::{AttributeValue, EndianSlice, LittleEndian, SectionId};
use object::{Object, ObjectSection, SectionKind};
use regex::Regex;

use crate::corpus::{Cpu, split_top};

#[derive(Clone)]
pub struct InlineSpec {
    pub target: String,
    pub items: Vec<Item>,
}

#[derive(Clone)]
pub struct Item {
    pub text: String,
    pub selector: Selector,
    pub context: Option<Context>,
}

#[derive(Clone, Copy)]
pub struct Context {
    pub up: usize,
    pub down: usize,
}

static CONTEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*?)(?:~~(\d+)|~(\d+)(?:\((\d+)\^\))?|\((\d+)\^\))$").unwrap());

impl Context {
    fn split(s: &str) -> Result<(&str, Option<Context>), String> {
        let Some(c) = CONTEXT.captures(s) else { return Ok((s, None)) };
        let num = |i: usize| -> Result<usize, String> {
            c.get(i).map_or(Ok(0), |m| m.as_str().parse().map_err(|_| format!("'{s}': context line count too large")))
        };
        let context = match (c.get(2), c.get(3)) {
            (Some(_), _) => Context { up: num(2)?, down: num(2)? },
            (_, Some(_)) => Context { up: num(4)?, down: num(3)? },
            _ => Context { up: num(5)?, down: 0 },
        };
        Ok((c.get(1).unwrap().as_str().trim(), Some(context)))
    }
}

#[derive(Clone)]
pub enum Selector {
    Named(String, Option<Span>),
    All(Span, Option<Span>),
    Flat(Span),
}

#[derive(Clone, Copy)]
pub struct Span {
    lo: usize,
    hi: Option<usize>,
}

impl Span {
    const ALL: Span = Span { lo: 0, hi: None };

    fn at(k: usize) -> Span {
        Span { lo: k, hi: Some(k + 1) }
    }

    pub(crate) fn pick(self, len: usize) -> Range<usize> {
        let hi = self.hi.unwrap_or(len).min(len);
        self.lo.min(hi)..hi
    }

    pub(crate) fn parse(s: &str) -> Result<Span, String> {
        let num = |t: &str| {
            t.trim().parse::<usize>().map_err(|_| format!("'{s}' is not an index or a range (k, a..b, a..=b, a.., ..b, ..)"))
        };
        let Some((a, b)) = s.split_once("..") else {
            return if s.is_empty() { Ok(Span::ALL) } else { num(s).map(Span::at) };
        };
        let lo = if a.trim().is_empty() { 0 } else { num(a)? };
        let hi = match b.trim() {
            "" => None,
            b => Some(match b.strip_prefix('=') {
                Some(end) => num(end)? + 1,
                None => num(b)?,
            }),
        };
        Ok(Span { lo, hi })
    }
}

impl Selector {
    fn parse(s: &str) -> Result<Selector, String> {
        let (head, dims): (&str, Vec<&str>) = match s.strip_suffix(']').and_then(|r| r.split_once('[')) {
            Some((head, inner)) => (head.trim(), inner.split(',').map(str::trim).collect()),
            None => (s, Vec::new()),
        };
        let one = || match dims.as_slice() {
            [] => Ok(None),
            [d] => Span::parse(d).map(Some),
            _ => Err(format!("'{s}': {head}[...] takes one index or range")),
        };
        Ok(match head {
            "..all" => Selector::Flat(one()?.unwrap_or(Span::ALL)),
            "all" => match dims.as_slice() {
                [] => Selector::All(Span::ALL, None),
                [i] => Selector::All(Span::parse(i)?, None),
                [i, ""] => Selector::All(Span::parse(i)?, None),
                [i, j] => Selector::All(Span::parse(i)?, Some(Span::parse(j)?)),
                _ => return Err(format!("'{s}': all[...] takes at most two dimensions, all[symbols, instances]")),
            },
            name if !name.is_empty() && !name.starts_with("..") && !name.contains(|c: char| c.is_whitespace() || "()[],=~^".contains(c)) => {
                Selector::Named(name.to_string(), one()?)
            }
            _ => {
                return Err(format!(
                    "'{s}' is not a selector (expected SYMBOL, SYMBOL[j], all, all[i], all[i, j], ..all or ..all[k]; \
                     indexes may be ranges a..b, a..=b, a.., ..b; context suffixes are ~N, ~~N, ~N(M^), (M^))"
                ));
            }
        })
    }
}

impl InlineSpec {
    pub fn parse(inner: &str) -> Result<InlineSpec, String> {
        let parts = split_top(inner);
        let target = parts[0]
            .strip_prefix('=')
            .map(str::trim)
            .filter(|t| !t.is_empty() && !t.contains(|c: char| c.is_whitespace() || "()[],".contains(c)))
            .ok_or_else(|| format!("inline({inner}): the first item must be =SYMBOL, the inlined function"))?;
        let mut items = Vec::new();
        for p in &parts[1..] {
            let item = Context::split(p).and_then(|(body, context)| {
                Ok(Item { text: body.to_string(), selector: Selector::parse(body)?, context })
            });
            items.push(item.map_err(|e| format!("inline({inner}): {e}"))?);
        }
        if items.is_empty() {
            items.push(Item { text: "..all[0]".to_string(), selector: Selector::Flat(Span::at(0)), context: None });
        }
        Ok(InlineSpec { target: target.to_string(), items })
    }

    pub fn callers(&self) -> impl Iterator<Item = &str> {
        self.items.iter().filter_map(|i| match &i.selector {
            Selector::Named(c, _) => Some(c.as_str()),
            _ => None,
        })
    }

    pub fn cpu_hint(&self) -> Option<Cpu> {
        std::iter::once(self.target.as_str()).chain(self.callers()).find_map(|n| Cpu::split(n).0)
    }

    pub fn caller_query(&self, caller: &str) -> String {
        match (Cpu::split(caller).0, self.cpu_hint()) {
            (None, Some(cpu)) => format!("{}/{caller}", cpu.label()),
            _ => caller.to_string(),
        }
    }
}

pub fn demangled(s: &str) -> String {
    format!("{:#}", rustc_demangle::demangle(s))
}

pub struct CallSite {
    file: String,
    line: u64,
}

impl fmt::Display for CallSite {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.file, self.line)?;
        if self.line == 0 {
            f.write_str(" (shared by several calls)")?;
        }
        Ok(())
    }
}

pub struct Instance {
    pub ranges: Vec<(u64, u64)>,
    pub call: Option<CallSite>,
    caller: Option<u64>,
}

struct Die {
    name: Option<String>,
    linkage: Option<String>,
    origin: Option<u64>,
}

pub struct Info {
    dies: HashMap<u64, Die>,
    order: Vec<(u64, Instance)>,
}

impl Info {
    pub fn load(data: &[u8]) -> Result<Info> {
        let file = object::File::parse(data)?;
        Info::read(&file).context("reading DWARF")
    }

    fn read(file: &object::File) -> Result<Info> {
        let exec: Vec<(u64, u64)> = file
            .sections()
            .filter(|s| s.kind() == SectionKind::Text && s.address() != 0 && s.size() != 0)
            .map(|s| (s.address(), s.address() + s.size()))
            .collect();
        let dwarf = gimli::Dwarf::load(|id: SectionId| -> Result<_, gimli::Error> {
            let data = file.section_by_name(id.name()).and_then(|s| s.data().ok()).unwrap_or(&[]);
            Ok(EndianSlice::new(data, LittleEndian))
        })?;
        let mut info = Info { dies: HashMap::new(), order: Vec::new() };
        let mut units = dwarf.units();
        while let Some(header) = units.next()? {
            let unit = dwarf.unit(header)?;
            let global = |off: gimli::UnitOffset| off.to_debug_info_offset(&unit.header).map(|o| o.0 as u64);
            let file_name = |index: u64| -> Option<String> {
                let header = unit.line_program.as_ref()?.header();
                let path = dwarf.attr_string(&unit, header.file(index)?.path_name()).ok()?.to_string_lossy();
                Some(path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string())
            };
            let mut subprograms: Vec<(isize, u64)> = Vec::new();
            let mut cursor = unit.entries();
            while let Some(entry) = cursor.next_dfs()? {
                let depth = entry.depth();
                while subprograms.last().is_some_and(|(d, _)| *d >= depth) {
                    subprograms.pop();
                }
                let tag = entry.tag();
                let Some(offset) = global(entry.offset()) else { continue };
                if tag != gimli::DW_TAG_subprogram && tag != gimli::DW_TAG_inlined_subroutine {
                    continue;
                }
                let string = |at| {
                    let value = dwarf.attr_string(&unit, entry.attr_value(at)?).ok()?;
                    Some(value.to_string_lossy().into_owned())
                };
                let reference = |at| match entry.attr_value(at) {
                    Some(AttributeValue::UnitRef(o)) => global(o),
                    Some(AttributeValue::DebugInfoRef(o)) => Some(o.0 as u64),
                    _ => None,
                };
                let origin = reference(gimli::DW_AT_abstract_origin).or_else(|| reference(gimli::DW_AT_specification));
                info.dies.insert(offset, Die { name: string(gimli::DW_AT_name), linkage: string(gimli::DW_AT_linkage_name), origin });
                if tag == gimli::DW_TAG_subprogram {
                    subprograms.push((depth, offset));
                    continue;
                }
                let mut ranges = Vec::new();
                if let Ok(mut iter) = dwarf.die_ranges(&unit, entry) {
                    while let Ok(Some(r)) = iter.next() {
                        if r.begin < r.end && exec.iter().any(|(lo, hi)| r.begin >= *lo && r.end <= *hi) {
                            ranges.push((r.begin, r.end));
                        }
                    }
                }
                if ranges.is_empty() {
                    continue;
                }
                ranges.sort();
                let call = match (entry.attr_value(gimli::DW_AT_call_file), entry.attr_value(gimli::DW_AT_call_line)) {
                    (Some(AttributeValue::FileIndex(f)), Some(l)) => {
                        l.udata_value().and_then(|line| Some(CallSite { file: file_name(f)?, line }))
                    }
                    _ => None,
                };
                info.order.push((offset, Instance { ranges, call, caller: subprograms.last().map(|(_, o)| *o) }));
            }
        }
        Ok(info)
    }

    fn origins(&self, off: u64) -> impl Iterator<Item = (u64, &Die)> {
        let mut seen = BTreeSet::new();
        let mut next = Some(off);
        std::iter::from_fn(move || {
            let cur = next.take().filter(|o| seen.insert(*o))?;
            let die = self.dies.get(&cur)?;
            next = die.origin;
            Some((cur, die))
        })
    }

    fn root(&self, off: u64) -> u64 {
        self.origins(off).last().map_or(off, |(o, _)| o)
    }

    fn symbol_of(&self, off: u64) -> Option<String> {
        let dies: Vec<&Die> = self.origins(off).map(|(_, d)| d).collect();
        dies.iter().find_map(|d| d.linkage.clone()).or_else(|| dies.iter().find_map(|d| d.name.clone()))
    }

    fn names_of(&self, root: u64) -> Vec<String> {
        self.origins(root)
            .flat_map(|(_, d)| {
                d.name.iter().cloned().chain(d.linkage.iter().flat_map(|l| [l.clone(), demangled(l)]))
            })
            .collect()
    }

    pub fn find(&self, query: &str) -> Result<Option<Target<'_>>, String> {
        let roots: BTreeSet<u64> = self
            .dies
            .iter()
            .filter(|(off, d)| d.origin.is_none() && self.names_of(**off).iter().any(|n| n == query))
            .map(|(off, _)| *off)
            .collect();
        let paths: BTreeSet<String> = roots
            .iter()
            .map(|r| self.origins(*r).find_map(|(_, d)| d.linkage.as_deref().map(demangled)).or_else(|| self.symbol_of(*r)).unwrap_or_default())
            .collect();
        if paths.len() > 1 {
            return Err(format!("'{query}' names several functions; write one of: {}", paths.into_iter().collect::<Vec<_>>().join(", ")));
        }
        let mut target = Target { name: query.to_string(), instances: Vec::new(), symbols: Vec::new() };
        for (off, inst) in &self.order {
            if !roots.contains(&self.root(*off)) {
                continue;
            }
            let name = inst.caller.and_then(|c| self.symbol_of(c)).unwrap_or_else(|| "?".to_string());
            let sym = match target.symbols.iter().position(|s| s.name == name) {
                Some(i) => i,
                None => {
                    target.symbols.push(Symbol { name, instances: Vec::new() });
                    target.symbols.len() - 1
                }
            };
            let j = target.symbols[sym].instances.len();
            target.symbols[sym].instances.push(target.instances.len());
            target.instances.push(Hit { inst, sym, j });
        }
        Ok((!target.instances.is_empty()).then_some(target))
    }
}

pub struct Symbol {
    pub name: String,
    pub instances: Vec<usize>,
}

pub struct Hit<'a> {
    pub inst: &'a Instance,
    pub sym: usize,
    pub j: usize,
}

pub struct Target<'a> {
    pub name: String,
    pub instances: Vec<Hit<'a>>,
    pub symbols: Vec<Symbol>,
}

pub enum Pick {
    Instance(usize),
    Whole(usize),
    Absent { whole: bool },
    Nothing(String),
}

impl Target<'_> {
    pub fn symbol_list(&self) -> String {
        self.symbols.iter().map(|s| demangled(&s.name)).collect::<Vec<_>>().join(", ")
    }

    fn symbol(&self, sel: &str) -> Option<usize> {
        let sel = Cpu::split(sel).1;
        self.symbols.iter().position(|s| s.name == sel || demangled(&s.name) == sel)
    }

    fn instances_of(&self, sym: usize, span: Span) -> impl Iterator<Item = Pick> + '_ {
        let instances = &self.symbols[sym].instances;
        span.pick(instances.len()).map(|j| Pick::Instance(instances[j]))
    }

    pub fn select(&self, sel: &Selector) -> Vec<Pick> {
        let (n, g, name) = (self.instances.len(), self.symbols.len(), &self.name);
        let (picks, why): (Vec<Pick>, String) = match sel {
            Selector::Named(s, span) => match (self.symbol(s), span) {
                (None, _) => return vec![Pick::Absent { whole: span.is_none() }],
                (Some(i), None) => return vec![Pick::Whole(i)],
                (Some(i), Some(span)) => {
                    let sym = &self.symbols[i];
                    let why = format!("{} inlines {name} {} time(s)", demangled(&sym.name), sym.instances.len());
                    (self.instances_of(i, *span).collect(), why)
                }
            },
            Selector::All(syms, insts) => {
                let picks = match insts {
                    None => syms.pick(g).map(Pick::Whole).collect(),
                    Some(span) => syms.pick(g).flat_map(|i| self.instances_of(i, *span)).collect(),
                };
                (picks, format!("{name} is inlined {n} time(s) in {g} symbol(s): {}", self.symbol_list()))
            }
            Selector::Flat(span) => (span.pick(n).map(Pick::Instance).collect(), format!("{name} is inlined {n} time(s)")),
        };
        if picks.is_empty() { vec![Pick::Nothing(why)] } else { picks }
    }
}
