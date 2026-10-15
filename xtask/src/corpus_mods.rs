use std::collections::BTreeSet;
use std::ops::Range;

use crate::corpus::{INSN, LABEL, Labels, MARK_TAG, split_top};
use crate::corpus_inline::Span;

const MERGED: &str = "--- filter ranges merged ---";

#[derive(Clone, Default)]
pub struct Mods {
    pub ignore: bool,
    pub lines: Lines,
}

#[derive(Clone, Default)]
pub struct Lines {
    ops: Vec<Op>,
}

#[derive(Clone, Copy)]
struct Op {
    show: bool,
    span: Span,
}

enum Kind {
    Asm,
    Label,
    Gap,
    Other,
}

impl Kind {
    fn of(line: &str) -> Kind {
        if LABEL.is_match(line) || INSN.is_match(line) {
            Kind::Asm
        } else if line.starts_with(MARK_TAG) {
            Kind::Label
        } else if line == "..." {
            Kind::Gap
        } else {
            Kind::Other
        }
    }
}

impl Mods {
    pub fn split(item: &str) -> Result<(&str, Mods), String> {
        let Some(open) = item.find('{') else { return Ok((item, Mods::default())) };
        let inner = item[open + 1..]
            .strip_suffix('}')
            .ok_or_else(|| format!("'{item}': all modifiers must be in a single {{...}} after the item"))?;
        let mut mods = Mods::default();
        for m in split_top(inner) {
            if m.is_empty() {
                return Err(format!("'{item}': empty modifier"));
            } else if m == "ignore" {
                mods.ignore = true;
            } else if let Some(ops) = m.strip_prefix("lines(").and_then(|r| r.strip_suffix(')')) {
                mods.lines.parse(ops).map_err(|e| format!("'{item}': {e}"))?;
            } else {
                return Err(format!("'{item}': unknown modifier '{m}' (expected one of: `ignore`, `lines(...)`)"));
            }
        }
        Ok((item[..open].trim(), mods))
    }
}

fn filtered_out(n: usize) -> String {
    format!("... {n} line{} filtered out by modifiers ...", if n == 1 { "" } else { "s" })
}

impl Lines {
    fn parse(&mut self, s: &str) -> Result<(), String> {
        let mut show = true;
        for op in split_top(s) {
            let (word, rest) = op.split_once(char::is_whitespace).unwrap_or((op, ""));
            let range = match word {
                "show" | "hide" => {
                    show = word == "show";
                    rest.trim()
                }
                _ => op,
            };
            if range.is_empty() {
                return Err(format!("lines({s}): '{op}' must be a range (k, a..b, a..=b, a.., ..b, ..)"));
            }
            self.ops.push(Op { show, span: Span::parse(range)? });
        }
        Ok(())
    }

    fn ranges(&self, n: usize) -> Vec<Range<usize>> {
        let mut kept = match self.ops.first() {
            Some(Op { show: false, .. }) => vec![0..n],
            _ => Vec::new(),
        };
        for op in &self.ops {
            let r = op.span.pick(n);
            if r.is_empty() {
                continue;
            }
            if op.show {
                kept.push(r);
            } else {
                kept = kept
                    .into_iter()
                    .flat_map(|k| [k.start..k.end.min(r.start), k.start.max(r.end)..k.end])
                    .filter(|k| !k.is_empty())
                    .collect();
            }
        }
        kept.sort_by_key(|r| r.start);
        kept
    }

    fn shown(&self, n: usize) -> (Vec<bool>, BTreeSet<usize>) {
        let mut shown = vec![false; n];
        let mut merges = BTreeSet::new();
        let mut end: Option<usize> = None;
        for r in self.ranges(n) {
            shown[r.clone()].fill(true);
            if let Some(e) = end
                && r.start <= e
                && r.end > e
            {
                merges.insert(e);
            }
            end = Some(end.map_or(r.end, |e| e.max(r.end)));
        }
        (shown, merges)
    }

    pub fn apply(&self, lines: Vec<String>, labels: Labels) -> Vec<String> {
        if self.ops.is_empty() {
            return lines;
        }
        let n = lines.iter().filter(|l| matches!(Kind::of(l), Kind::Asm)).count();
        let (shown, mut merges) = self.shown(n);
        let mut out = Vec::new();
        let mut hidden = 0;
        let mut k: usize = 0;
        for line in lines {
            let (prev, next) = (k.checked_sub(1), (k < n).then_some(k));
            let kind = Kind::of(&line);
            let owner = match kind {
                Kind::Asm => {
                    k += 1;
                    next
                }
                Kind::Label if labels == Labels::Before => next.or(prev),
                Kind::Label => prev.or(next),
                Kind::Gap => {
                    if prev.zip(next).is_some_and(|(p, q)| shown[p] && shown[q]) {
                        out.push(line);
                    }
                    continue;
                }
                Kind::Other => None,
            };
            if owner.is_some_and(|o| !shown[o]) {
                hidden += usize::from(matches!(kind, Kind::Asm));
                continue;
            }
            if hidden > 0 {
                out.push(filtered_out(hidden));
                hidden = 0;
            }
            if owner.is_some_and(|o| merges.remove(&o)) {
                out.push(MERGED.to_string());
            }
            out.push(line);
        }
        if hidden > 0 {
            out.push(filtered_out(hidden));
        }
        out
    }
}
