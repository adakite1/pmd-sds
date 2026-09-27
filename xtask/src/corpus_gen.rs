use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use anyhow::{Context, Result, bail, ensure};
use regex::Regex;
use serde::{Deserialize, Serialize};

const OBJDUMP: &str = "arm-none-eabi-objdump";
const REGIONS: [&str; 3] = ["us", "eu", "jp"];
const ROW_NAMES: [&str; 10] = [".text", ".itcm", ".dtcm", ".arm7", ".wram", ".main", ".bss", ".rodata", ".data", ".init"];
const ARM7_MODULES: [&str; 3] = ["arm7", "WRAM", "MAIN"];
const SOURCE_SKIP_DIRS: [&str; 3] = ["build", ".git", "tools"];
pub const MARKER_FILE: &str = ".asm-corpus";

static HDR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^# \.(\S+)").unwrap());
static SYM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^  ([0-9a-fA-F]{8}) ([0-9a-fA-F]{8}) (\S+)\s+(\S+)\t\((.*)\)\s*$").unwrap()
});
static LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([0-9a-f]{8}) <([^>]+)>:").unwrap());
static INSN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*([0-9a-f]+):\t([0-9a-f ]+)\t(.*)$").unwrap());
static OBJ: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\S+\.o)").unwrap());
static UNSAFE_CHARS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^A-Za-z0-9_.@-]").unwrap());

fn arm9_modules() -> Vec<String> {
    let mut m: Vec<String> = ["main", "ITCM", "DTCM"].map(String::from).to_vec();
    m.extend((0..36).map(|i| format!("OVY_{i}")));
    m
}

fn code_sub(module: &str) -> &'static str {
    match module {
        "ITCM" => ".itcm",
        "DTCM" => ".dtcm",
        _ => ".text",
    }
}

fn dirname(module: &str) -> String {
    match module {
        "arm7" => "static".into(),
        "WRAM" => "wram".into(),
        "MAIN" => "main".into(),
        "ITCM" => "itcm".into(),
        "DTCM" => "dtcm".into(),
        m => match m.strip_prefix("OVY_").and_then(|n| n.parse::<u32>().ok()) {
            Some(n) => format!("ov{n:02}"),
            None => m.into(),
        },
    }
}

fn sanitize(name: &str) -> String {
    UNSAFE_CHARS.replace_all(name, "_").into_owned()
}

struct Rom {
    d: Vec<u8>,
}

struct Overlay {
    ram: u64,
    size: u64,
    file_id: u32,
}

impl Rom {
    fn open(path: &Path) -> Result<Rom> {
        Ok(Rom { d: fs::read(path).with_context(|| format!("reading {}", path.display()))? })
    }

    fn u32(&self, off: usize) -> u32 {
        u32::from_le_bytes(self.d[off..off + 4].try_into().unwrap())
    }

    fn fat_entry(&self, file_id: u32) -> (usize, usize) {
        let off = self.u32(0x48) as usize + 8 * file_id as usize;
        (self.u32(off) as usize, self.u32(off + 4) as usize)
    }

    fn overlays(&self) -> HashMap<u32, Overlay> {
        let (off, size) = (self.u32(0x50) as usize, self.u32(0x54) as usize);
        (0..size / 32)
            .map(|i| {
                let e = off + 32 * i;
                (self.u32(e), Overlay { ram: self.u32(e + 4) as u64, size: self.u32(e + 8) as u64, file_id: self.u32(e + 24) })
            })
            .collect()
    }
}

#[derive(Clone)]
struct Row {
    addr: u64,
    size: u64,
    sub: String,
    name: String,
    obj: String,
}

fn parse_xmap(path: &Path) -> Result<HashMap<String, Vec<Row>>> {
    let text = String::from_utf8_lossy(&fs::read(path).with_context(|| format!("reading {}", path.display()))?).into_owned();
    let mut sections: HashMap<String, Vec<Row>> = HashMap::new();
    let mut cur: Option<String> = None;
    for line in text.split('\n') {
        let line = line.trim_end_matches(['\r', '\n']);
        if let Some(c) = HDR.captures(line) {
            cur = Some(c[1].to_string());
            continue;
        }
        if let (Some(c), Some(cur)) = (SYM.captures(line), cur.as_ref()) {
            sections.entry(cur.clone()).or_default().push(Row {
                addr: u64::from_str_radix(&c[1], 16)?,
                size: u64::from_str_radix(&c[2], 16)?,
                sub: c[3].to_string(),
                name: c[4].to_string(),
                obj: c[5].trim().to_string(),
            });
        }
    }
    Ok(sections)
}

fn mod_bytes(rom: &Rom, module: &str) -> Result<(u64, Vec<u8>)> {
    let arm9_off = rom.u32(0x20) as usize;
    let arm9_size = rom.u32(0x2c) as usize;
    let table = arm9_off + arm9_size - 0x18;
    let (itcm_dest, itcm_size) = (rom.u32(table) as u64, rom.u32(table + 4) as usize);
    let (dtcm_dest, dtcm_size) = (rom.u32(table + 12) as u64, rom.u32(table + 16) as usize);
    let static_size = arm9_size - 0x18 - itcm_size - dtcm_size;
    Ok(match module {
        "main" => (0x0200_0000, rom.d[arm9_off..arm9_off + static_size].to_vec()),
        "ITCM" => (itcm_dest, rom.d[arm9_off + static_size..arm9_off + static_size + itcm_size].to_vec()),
        "DTCM" => {
            let start = arm9_off + static_size + itcm_size;
            (dtcm_dest, rom.d[start..start + dtcm_size].to_vec())
        }
        m => {
            let id: u32 = m.strip_prefix("OVY_").context("unknown module")?.parse()?;
            let overlays = rom.overlays();
            let ent = overlays.get(&id).with_context(|| format!("{m}: no overlay table entry"))?;
            let (start, end) = rom.fat_entry(ent.file_id);
            ensure!((end - start) as u64 == ent.size, "{m}: overlay size mismatch");
            (ent.ram, rom.d[start..end].to_vec())
        }
    })
}

fn arm7_mod_bytes(rom: &Rom) -> Result<HashMap<&'static str, (u64, Vec<u8>)>> {
    let off = rom.u32(0x30) as usize;
    ensure!(rom.u32(0x3c) == 0x1E8 + 0xF608 + 0x17878 + 0x18, "unexpected ARM7 binary size");
    let d = &rom.d[off..off + 0x27080];
    Ok(HashMap::from([
        ("arm7", (0x0238_0000, d[..0x1E8].to_vec())),
        ("WRAM", (0x037F_8000, d[0x1E8..0x1E8 + 0xF608].to_vec())),
        ("MAIN", (0x027E_0000, d[0x0F7F0..0x0F7F0 + 0x17878].to_vec())),
    ]))
}

#[derive(Clone)]
struct Func {
    addr: u64,
    size: u64,
    end: u64,
    raw_size: u64,
    bogus: bool,
    name: String,
    aliases: Vec<String>,
    obj: String,
    lines: Vec<String>,
}

fn collect_functions(rows: &[Row], sub: &str) -> (Vec<Func>, Option<u64>) {
    let code_rows: Vec<&Row> = rows.iter().filter(|r| r.sub == sub).collect();
    let code_start = code_rows.iter().map(|r| r.addr).min();
    let mut order: Vec<u64> = Vec::new();
    let mut byaddr: HashMap<u64, Vec<&Row>> = HashMap::new();
    for r in code_rows
        .into_iter()
        .filter(|r| r.size > 0 && !r.name.starts_with('$') && !ROW_NAMES.contains(&r.name.as_str()))
    {
        byaddr
            .entry(r.addr)
            .or_insert_with(|| {
                order.push(r.addr);
                Vec::new()
            })
            .push(r);
    }
    let mut funcs: Vec<Func> = order
        .iter()
        .map(|a| {
            let group = &byaddr[a];
            let p = group[0];
            Func {
                addr: *a,
                size: p.size,
                end: 0,
                raw_size: p.size,
                bogus: false,
                name: p.name.clone(),
                aliases: group[1..].iter().map(|g| g.name.clone()).collect(),
                obj: p.obj.clone(),
                lines: Vec::new(),
            }
        })
        .collect();
    funcs.sort_by_key(|f| f.addr);
    for i in 0..funcs.len() {
        let nxt = funcs.get(i + 1).map(|f| f.addr);
        let f = &mut funcs[i];
        let end = f.addr + f.size;
        if end <= f.addr || f.size > 0x40_0000 || nxt.is_some_and(|n| end > n) {
            f.bogus = true;
            if let Some(n) = nxt {
                f.size = n - f.addr;
            }
        }
        f.end = f.addr + f.size;
    }
    (funcs, code_start)
}

fn literal_targets(data: &[u8], base: u64, lo: u64, hi: u64) -> HashSet<u64> {
    let mut out = HashSet::new();
    let first = lo.saturating_sub(base);
    let mut off = first.div_ceil(4) * 4;
    while (off as usize) + 3 < data.len() && base + off < hi {
        let addr = base + off;
        let w = u32::from_le_bytes(data[off as usize..off as usize + 4].try_into().unwrap());
        if w & 0x0F5F_0000 == 0x051F_0000 {
            let imm = (w & 0xFFF) as u64;
            out.insert(if w & 0x0080_0000 != 0 { addr + 8 + imm } else { (addr + 8).wrapping_sub(imm) });
        }
        off += 4;
    }
    let mut off = first.div_ceil(2) * 2;
    while (off as usize) + 1 < data.len() && base + off < hi {
        let addr = base + off;
        let h = u16::from_le_bytes(data[off as usize..off as usize + 2].try_into().unwrap());
        if h & 0xF800 == 0x4800 {
            out.insert((addr & !3) + 4 + (h & 0xFF) as u64 * 4);
        }
        off += 2;
    }
    out
}

fn extend_literal_pools(funcs: &mut [Func], base: u64, data: &[u8]) {
    funcs.sort_by_key(|f| f.addr);
    for i in 0..funcs.len() {
        let limit = funcs.get(i + 1).map_or(base + data.len() as u64, |f| f.addr);
        let f = &mut funcs[i];
        let mut end = f.end;
        if end >= limit || end % 4 != 0 {
            continue;
        }
        let targets = literal_targets(data, base, f.addr, end);
        while end + 4 <= limit && targets.contains(&end) {
            end += 4;
        }
        f.end = end;
        f.size = end - f.addr;
    }
}

fn compute_gaps(funcs: &[Func], lo: u64, hi: u64) -> Vec<(u64, u64)> {
    let mut sorted: Vec<&Func> = funcs.iter().collect();
    sorted.sort_by_key(|f| f.addr);
    let mut gaps = Vec::new();
    let mut cur = lo;
    for f in sorted {
        if f.addr > cur {
            gaps.push((cur, f.addr));
        }
        cur = cur.max(f.end);
    }
    if cur < hi {
        gaps.push((cur, hi));
    }
    gaps
}

fn build_source_index(decomp: &Path) -> Result<HashMap<String, Vec<String>>> {
    fn walk(dir: &Path, decomp: &Path, idx: &mut HashMap<String, Vec<String>>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let ft = entry.file_type()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if ft.is_dir() {
                if !SOURCE_SKIP_DIRS.contains(&name.as_str()) {
                    walk(&path, decomp, idx)?;
                }
                continue;
            }
            if ft.is_symlink() && path.is_dir() {
                continue;
            }
            if name.ends_with(".c") || name.ends_with(".s") {
                let stem = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s).to_lowercase();
                let rel = path.strip_prefix(decomp).unwrap().to_string_lossy().replace('\\', "/");
                idx.entry(stem).or_default().push(rel);
            }
        }
        Ok(())
    }
    let mut idx = HashMap::new();
    walk(decomp, decomp, &mut idx)?;
    Ok(idx)
}

fn pick_source(idx: &HashMap<String, Vec<String>>, obj: &str) -> Option<String> {
    let last = OBJ.find_iter(obj).last()?.as_str();
    let cands = idx.get(&last[..last.len() - 2].to_lowercase())?;
    let score = |rel: &String| {
        let ext = if rel.ends_with(".c") { 0 } else { 1 };
        let pref = if rel.starts_with("src/") {
            0
        } else if rel.starts_with("lib/src/") {
            1
        } else if rel.starts_with("asm/") {
            2
        } else {
            3
        };
        (ext, pref, rel.clone())
    };
    cands.iter().min_by_key(|r| score(r)).cloned()
}

struct Sym {
    addr: u64,
    size: u64,
    name: String,
    func: bool,
}

fn mini_symbols(current: &[Row], lower_groups: &[Vec<&Row>], lo: u64, hi: u64) -> Vec<Sym> {
    let mut map_seen = HashSet::new();
    let mut sym_seen = HashSet::new();
    let mut out = Vec::new();
    let mut add = |r: &Row, restrict_outside: bool| {
        let name = r.name.as_str();
        if ROW_NAMES.contains(&name) || name.starts_with('.') {
            return;
        }
        if restrict_outside && lo <= r.addr && r.addr < hi {
            return;
        }
        let size = if r.size <= 0x10_0000 { r.size } else { 0 };
        if name.starts_with('$') {
            if !matches!(name, "$a" | "$t" | "$d") || !map_seen.insert((r.addr, name.to_string())) {
                return;
            }
            out.push(Sym { addr: r.addr, size: 0, name: name.to_string(), func: false });
        } else {
            if !sym_seen.insert(r.addr) {
                return;
            }
            out.push(Sym { addr: r.addr, size, name: name.to_string(), func: size > 0 });
        }
    };
    for r in current {
        add(r, false);
    }
    for group in lower_groups {
        for r in group {
            add(r, true);
        }
    }
    out.sort_by_key(|s| s.addr);
    out
}

fn write_mini_elf(path: &Path, base: u64, code: &[u8], symbols: &[Sym]) -> Result<()> {
    let mut strtab = vec![0u8];
    let mut offsets: HashMap<&str, u32> = HashMap::new();
    let mut symtab = vec![0u8; 16];
    for s in symbols {
        let name_off = *offsets.entry(&s.name).or_insert_with(|| {
            let o = strtab.len() as u32;
            strtab.extend_from_slice(s.name.as_bytes());
            strtab.push(0);
            o
        });
        symtab.extend_from_slice(&name_off.to_le_bytes());
        symtab.extend_from_slice(&(s.addr as u32).to_le_bytes());
        symtab.extend_from_slice(&(s.size as u32).to_le_bytes());
        symtab.push(if s.func { 0x02 } else { 0x00 });
        symtab.push(0);
        symtab.extend_from_slice(&1u16.to_le_bytes());
    }
    let align4 = |x: usize| (x + 3) & !3;
    let text_off = 52;
    let symtab_off = align4(text_off + code.len());
    let strtab_off = align4(symtab_off + symtab.len());
    let shstr_off = align4(strtab_off + strtab.len());
    let mut shstr = vec![0u8];
    let mut names = HashMap::new();
    for nm in [".text", ".symtab", ".strtab", ".shstrtab"] {
        names.insert(nm, shstr.len() as u32);
        shstr.extend_from_slice(nm.as_bytes());
        shstr.push(0);
    }
    let shoff = align4(shstr_off + shstr.len());
    let shdr = |v: [u32; 10]| v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>();

    let mut out = Vec::new();
    out.extend_from_slice(b"\x7fELF\x01\x01\x01\0\0\0\0\0\0\0\0\0");
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&40u16.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(shoff as u32).to_le_bytes());
    out.extend_from_slice(&0x0500_0000u32.to_le_bytes());
    for h in [52u16, 32, 0, 40, 5, 4] {
        out.extend_from_slice(&h.to_le_bytes());
    }
    out.extend_from_slice(code);
    out.resize(symtab_off, 0);
    out.extend_from_slice(&symtab);
    out.resize(strtab_off, 0);
    out.extend_from_slice(&strtab);
    out.resize(shstr_off, 0);
    out.extend_from_slice(&shstr);
    out.resize(shoff, 0);
    out.extend_from_slice(&[0u8; 40]);
    out.extend(shdr([names[".text"], 1, 0x6, base as u32, text_off as u32, code.len() as u32, 0, 0, 4, 0]));
    out.extend(shdr([names[".symtab"], 2, 0, 0, symtab_off as u32, symtab.len() as u32, 3, 1, 4, 16]));
    out.extend(shdr([names[".strtab"], 3, 0, 0, strtab_off as u32, strtab.len() as u32, 0, 0, 4, 0]));
    out.extend(shdr([names[".shstrtab"], 3, 0, 0, shstr_off as u32, shstr.len() as u32, 0, 0, 4, 0]));
    fs::write(path, out)?;
    Ok(())
}

fn parse_listing(elf: &Path) -> Result<Vec<(u64, String)>> {
    let out = Command::new(OBJDUMP).arg("-d").arg(elf).output().with_context(|| format!("running {OBJDUMP}"))?;
    ensure!(out.status.success(), "{OBJDUMP} failed on {}", elf.display());
    let text = String::from_utf8(out.stdout).context("objdump output is not UTF-8")?;
    let mut records: Vec<(u64, String)> = text
        .lines()
        .filter_map(|line| {
            let c = LABEL.captures(line).or_else(|| INSN.captures(line))?;
            Some((u64::from_str_radix(&c[1], 16).ok()?, line.to_string()))
        })
        .collect();
    records.sort_by_key(|r| r.0);
    Ok(records)
}

fn slice_records(records: Vec<(u64, String)>, funcs: &mut [Func]) {
    for f in funcs.iter_mut() {
        f.lines.clear();
    }
    let mut fi = 0;
    for (addr, text) in records {
        while fi < funcs.len() && addr >= funcs[fi].end {
            fi += 1;
        }
        if fi < funcs.len() && addr >= funcs[fi].addr {
            funcs[fi].lines.push(text);
        }
    }
}

struct Stats {
    funcs: usize,
    size: u64,
    gaps: Vec<(u64, u64)>,
    func_bytes: u64,
    first: Option<u64>,
    last: Option<u64>,
}

impl Stats {
    fn of(funcs: &[Func], size: u64, gaps: Vec<(u64, u64)>) -> Stats {
        Stats {
            funcs: funcs.len(),
            size,
            gaps,
            func_bytes: funcs.iter().map(|f| f.end - f.addr).sum(),
            first: funcs.first().map(|f| f.addr),
            last: funcs.iter().map(|f| f.end).max(),
        }
    }
}

struct Job {
    region: &'static str,
    family: &'static str,
    module: String,
    funcs: Vec<Func>,
    elf: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
struct OutEntry {
    addr: String,
    size: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    listing: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    same_as: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct OutRegions {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    us: Option<OutEntry>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    eu: Option<OutEntry>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    jp: Option<OutEntry>,
}

impl OutRegions {
    fn slot(&mut self, r: &str) -> &mut Option<OutEntry> {
        match r {
            "us" => &mut self.us,
            "eu" => &mut self.eu,
            _ => &mut self.jp,
        }
    }

    fn iter(&self) -> impl Iterator<Item = (&'static str, &OutEntry)> {
        [("us", &self.us), ("eu", &self.eu), ("jp", &self.jp)].into_iter().filter_map(|(r, e)| Some((r, e.as_ref()?)))
    }
}

#[derive(Serialize, Deserialize)]
struct OutRecord {
    name: String,
    module: String,
    source: String,
    aliases: Vec<String>,
    notes: Vec<String>,
    regions: OutRegions,
}

fn hexdump(data: &[u8], addr: u64) -> String {
    data.chunks(16)
        .enumerate()
        .map(|(i, chunk)| {
            let bytes: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
            format!("{:08x}: {}", addr + 16 * i as u64, bytes.join(" "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn slice_clamped(data: &[u8], start: u64, len: u64) -> &[u8] {
    let s = (start as usize).min(data.len());
    let e = ((start + len) as usize).min(data.len());
    &data[s..e]
}

fn to_json(record: &OutRecord) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, serde_json::ser::PrettyFormatter::with_indent(b" "));
    record.serialize(&mut ser)?;
    buf.push(b'\n');
    Ok(buf)
}

pub fn generate(decomp: &Path, out_root: &Path) -> Result<usize> {
    let rom_path = |r: &str| decomp.join(format!("build/pmdsky.{r}/pmdsky.{r}.nds"));
    let mut roms = HashMap::new();
    let mut xmaps = HashMap::new();
    for r in REGIONS {
        roms.insert(r, Rom::open(&rom_path(r))?);
        xmaps.insert(r, parse_xmap(&decomp.join(format!("build/pmdsky.{r}/main.nef.xMAP")))?);
    }
    xmaps.insert("shared", parse_xmap(&decomp.join("sub/build/arm7.nef.xMAP"))?);
    let arm7_bytes = arm7_mod_bytes(&roms["us"])?;
    let arm9_mods = arm9_modules();

    let workdir = std::env::temp_dir().join(format!("asm-corpus-gen-{}", std::process::id()));
    fs::create_dir_all(&workdir)?;

    let mut module_stats: HashMap<(String, String, String), Stats> = HashMap::new();
    let mut module_data: HashMap<(String, String), (u64, Vec<u8>)> = HashMap::new();
    let mut jobs: Vec<Job> = Vec::new();
    let empty = Vec::new();

    for region in REGIONS {
        let xm = &xmaps[region];
        let core_pool: Vec<&Row> =
            ["main", "ITCM", "DTCM"].iter().flat_map(|m| xm.get(*m).unwrap_or(&empty).iter()).collect();
        for module in &arm9_mods {
            let (base, data) = mod_bytes(&roms[region], module)?;
            let rows = xm.get(module).unwrap_or(&empty);
            let (mut funcs, code_start) = collect_functions(rows, code_sub(module));
            extend_literal_pools(&mut funcs, base, &data);
            let gaps = compute_gaps(&funcs, base, base + data.len() as u64);
            module_stats.insert(
                (region.into(), "arm9".into(), module.clone()),
                Stats::of(&funcs, data.len() as u64, gaps),
            );
            let mut elf = None;
            if let (Some(code_start), false) = (code_start, funcs.is_empty()) {
                let code_end = funcs.iter().map(|f| f.end).max().unwrap();
                let path = workdir.join(format!("{region}-{module}.elf"));
                let others: Vec<&Row> = arm9_mods
                    .iter()
                    .filter(|m| *m != module)
                    .flat_map(|m| xm.get(m).unwrap_or(&empty).iter())
                    .collect();
                let symbols = mini_symbols(rows, &[core_pool.clone(), others], base, base + data.len() as u64);
                write_mini_elf(&path, code_start, &data[(code_start - base) as usize..(code_end - base) as usize], &symbols)?;
                elf = Some(path);
            }
            module_data.insert((region.into(), module.clone()), (base, data));
            jobs.push(Job { region, family: "arm9", module: module.clone(), funcs, elf });
        }
    }
    let x7 = &xmaps["shared"];
    for module in ARM7_MODULES {
        let (base, data) = arm7_bytes[module].clone();
        let rows = x7.get(module).unwrap_or(&empty);
        let (mut funcs, code_start) = collect_functions(rows, ".text");
        extend_literal_pools(&mut funcs, base, &data);
        let gaps = compute_gaps(&funcs, base, base + data.len() as u64);
        module_stats.insert(("shared".into(), "arm7".into(), module.into()), Stats::of(&funcs, data.len() as u64, gaps));
        let mut elf = None;
        if let (Some(code_start), false) = (code_start, funcs.is_empty()) {
            let code_end = funcs.iter().map(|f| f.end).max().unwrap();
            let path = workdir.join(format!("shared-{module}.elf"));
            let others: Vec<&Row> =
                ARM7_MODULES.iter().filter(|m| **m != module).flat_map(|m| x7.get(*m).unwrap_or(&empty).iter()).collect();
            let symbols = mini_symbols(rows, &[others], base, base + data.len() as u64);
            write_mini_elf(&path, code_start, &data[(code_start - base) as usize..(code_end - base) as usize], &symbols)?;
            elf = Some(path);
        }
        module_data.insert(("shared".into(), module.into()), (base, data));
        jobs.push(Job { region: "shared", family: "arm7", module: module.into(), funcs, elf });
    }

    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    for chunk in jobs.chunks_mut(threads) {
        std::thread::scope(|s| -> Result<()> {
            let handles: Vec<_> = chunk
                .iter_mut()
                .filter(|j| j.elf.is_some())
                .map(|j| {
                    s.spawn(move || -> Result<()> {
                        let records = parse_listing(j.elf.as_ref().unwrap())?;
                        slice_records(records, &mut j.funcs);
                        Ok(())
                    })
                })
                .collect();
            for h in handles {
                h.join().map_err(|_| anyhow::anyhow!("objdump thread panicked"))??;
            }
            Ok(())
        })?;
    }
    let _ = fs::remove_dir_all(&workdir);

    type Key = (&'static str, String, String);
    let mut corpus: HashMap<Key, Vec<(&'static str, Func)>> = HashMap::new();
    let mut order: Vec<Key> = Vec::new();
    for job in jobs {
        if job.elf.is_none() {
            continue;
        }
        for f in job.funcs {
            let key = (job.family, job.module.clone(), f.name.clone());
            let entry = corpus.entry(key.clone()).or_insert_with(|| {
                order.push(key);
                Vec::new()
            });
            match entry.iter_mut().find(|(r, _)| *r == job.region) {
                Some(slot) => slot.1 = f,
                None => entry.push((job.region, f)),
            }
        }
    }

    for region in REGIONS {
        let rom = &roms[region];
        let off = rom.u32(0x20) as usize + rom.u32(0x2c) as usize - 0x18;
        module_data.insert((region.into(), "AUTOLOAD_FOOTER".into()), (off as u64, rom.d[off..off + 0x24].to_vec()));
        module_stats.insert(
            (region.into(), "arm9".into(), "AUTOLOAD_FOOTER".into()),
            Stats { funcs: 0, size: 0x24, gaps: vec![(off as u64, off as u64 + 0x24)], func_bytes: 0, first: None, last: None },
        );
    }
    let rom = &roms["us"];
    let off = rom.u32(0x30) as usize + 0x27068;
    module_data.insert(("shared".into(), "AUTOLOAD_FOOTER".into()), (off as u64, rom.d[off..off + 0x18].to_vec()));
    module_stats.insert(
        ("shared".into(), "arm7".into(), "AUTOLOAD_FOOTER".into()),
        Stats { funcs: 0, size: 0x18, gaps: vec![(off as u64, off as u64 + 0x18)], func_bytes: 0, first: None, last: None },
    );

    order.sort_by_cached_key(|k| {
        (if k.0 == "arm9" { 0 } else { 1 }, k.1.clone(), corpus[k].iter().map(|(_, f)| f.addr).min().unwrap())
    });

    let src_idx = build_source_index(decomp)?;
    let region_read = |region: &str, module: &str, addr: u64, size: u64| -> &[u8] {
        let (base, data) = &module_data[&(region.to_string(), module.to_string())];
        slice_clamped(data, addr - base, size)
    };

    fs::create_dir_all(out_root)?;
    let mut index_lines = vec!["family\tmodule\tname\tus\teu\tjp\tsource".to_string()];
    let mut written = 0;
    for key in &order {
        let (family, module, name) = key;
        let per_region = &corpus[key];
        let get = |r: &str| per_region.iter().find(|(x, _)| *x == r).map(|(_, f)| f);
        let f0 = get("us").unwrap_or(&per_region[0].1);
        let shared = get("shared");
        let (baseline, base_f, shown): (&str, &Func, Vec<&str>) = match shared {
            Some(f) => ("shared", f, vec!["shared"]),
            None => {
                let baseline = REGIONS.into_iter().find(|r| get(r).is_some()).unwrap();
                let base_f = get(baseline).unwrap();
                let base_bytes = region_read(baseline, module, base_f.addr, base_f.size);
                let same = |r: &str| {
                    let f = get(r).unwrap();
                    region_read(r, module, f.addr, f.size) == base_bytes
                };
                let shown = REGIONS.into_iter().filter(|r| get(r).is_some() && (*r == baseline || !same(r))).collect();
                (baseline, base_f, shown)
            }
        };

        let rel_src = pick_source(&src_idx, &f0.obj).unwrap_or_else(|| format!("object {}", f0.obj));
        let mut notes = Vec::new();
        if base_f.bogus {
            notes.push(format!("link-map size 0x{:X} invalid; range clamped to 0x{:X}", base_f.raw_size, base_f.size));
        }
        let mut regions = OutRegions { us: None, eu: None, jp: None };
        for r in REGIONS {
            let (f, own) = match shared {
                Some(f) => (f, r == "us"),
                None => match get(r) {
                    Some(f) => (f, shown.contains(&r)),
                    None => continue,
                },
            };
            *regions.slot(r) = Some(OutEntry {
                addr: format!("0x{:08X}", f.addr),
                size: format!("0x{:X}", f.size),
                listing: own.then(|| f.lines.clone()),
                same_as: (!own).then(|| if shared.is_some() { "us".to_string() } else { baseline.to_string() }),
            });
        }
        let record = OutRecord {
            name: name.clone(),
            module: format!("{family}/{}", dirname(module)),
            source: rel_src.clone(),
            aliases: base_f.aliases.clone(),
            notes,
            regions,
        };
        let dir = out_root.join(family).join(dirname(module));
        fs::create_dir_all(&dir)?;
        fs::write(dir.join(format!("{:08X}_{}.json", f0.addr, sanitize(name))), to_json(&record)?)?;
        written += 1;

        let cells: Vec<String> = match shared {
            Some(f) => vec![format!("0x{:08X}:0x{:X}", f.addr, f.size); 3],
            None => REGIONS
                .iter()
                .map(|r| get(r).map_or("-".to_string(), |f| format!("0x{:08X}:0x{:X}", f.addr, f.size)))
                .collect(),
        };
        index_lines.push(format!("{family}\t{module}\t{name}\t{}\t{rel_src}", cells.join("\t")));
    }
    fs::write(out_root.join("index.tsv"), index_lines.join("\n") + "\n")?;

    let mut gap_lines: Vec<String> = [
        "Every byte of every module that is not inside a function file.",
        "Each gap is classified as pre-code, inter-function, post-code data, or",
        "whole-module when the module has no function symbols at all.",
        "AUTOLOAD_FOOTER ranges are given as ROM file offsets (linker metadata",
        "that lives between the ARM9/ARM7 module images).",
        "",
    ]
    .map(String::from)
    .to_vec();
    let mut keys: Vec<&(String, String, String)> = module_stats.keys().collect();
    keys.sort_by(|a, b| (&a.1, &a.2, &a.0).cmp(&(&b.1, &b.2, &b.0)));
    for k in &keys {
        let (region, family, module) = k;
        let st = &module_stats[*k];
        let base = module_data[&(region.clone(), module.clone())].0;
        gap_lines.push(format!(
            "== {region} {family}/{module}  module 0x{base:08X}..0x{:08X} ({:#x} bytes)",
            base + st.size,
            st.size
        ));
        for (a, b) in &st.gaps {
            let data = region_read(region, module, *a, b - a);
            let kind = match (st.first, st.last) {
                (None, _) => "whole module (no function symbols)",
                (Some(first), _) if *b <= first => "pre-code",
                (_, Some(last)) if *a >= last => "post-code data",
                _ => "inter-function",
            };
            let head = format!("{a:08x}..{b:08x}  {:#x} bytes  [{kind}]", b - a);
            if b - a > 64 {
                gap_lines.push(format!("{head}\n{}", hexdump(data, *a)));
            } else {
                let hex: Vec<String> = data.iter().map(|x| format!("{x:02x}")).collect();
                gap_lines.push(format!("{head}: {}", hex.join(" ")));
            }
        }
        gap_lines.push(String::new());
    }
    fs::create_dir_all(out_root.join("_gaps"))?;
    fs::write(out_root.join("_gaps/all-gaps.txt"), gap_lines.join("\n"))?;

    build_coverage(decomp, out_root, &module_stats, &roms)?;
    fs::write(out_root.join(MARKER_FILE), "generated by cargo xtask corpus generate\n")?;
    Ok(written)
}

fn build_coverage(
    decomp: &Path,
    out_root: &Path,
    module_stats: &HashMap<(String, String, String), Stats>,
    roms: &HashMap<&str, Rom>,
) -> Result<()> {
    let mut lines: Vec<String> = [
        "# Corpus coverage",
        "",
        "Every function byte in this corpus is sliced directly from the retail-hash verified",
        ".nds files. Function boundaries come from the decomp link maps. \"code span\" is",
        "the range from the first to the last code symbol of a module; \"gap\" bytes are",
        "inside that span but not attributed to any function symbol (this includes padding, secure",
        "area, unlabelled data) and are dumped in _gaps/all-gaps.txt.",
        "",
        "The decompilation's build artifacts are compared with the retail ROM below.",
        "If all checks pass, the corpus's link-map ranges refer to bytes matching the retail ROM.",
        "",
        "## Function/gap accounting",
        "",
        "Every module byte is either inside a function file or inside a gap in",
        "_gaps/all-gaps.txt (the check column verifies this).",
        "",
        "| region | module | module bytes | funcs | function bytes | gaps | gap bytes | check |",
        "|---|---|---:|---:|---:|---:|---:|---|",
    ]
    .map(String::from)
    .to_vec();
    let mut keys: Vec<&(String, String, String)> = module_stats.keys().collect();
    keys.sort();
    let (mut t_size, mut t_func, mut t_gap, mut bad) = (0u64, 0u64, 0u64, 0usize);
    for k in keys {
        let (region, family, module) = k;
        let st = &module_stats[k];
        let gap_bytes: u64 = st.gaps.iter().map(|(a, b)| b - a).sum();
        let ok = st.func_bytes + gap_bytes == st.size;
        if !ok {
            bad += 1;
        }
        t_size += st.size;
        t_func += st.func_bytes;
        t_gap += gap_bytes;
        lines.push(format!(
            "| {region} | {family}/{module} | {:#x} | {} | {:#x} | {} | {gap_bytes:#x} | {} |",
            st.size,
            st.funcs,
            st.func_bytes,
            st.gaps.len(),
            if ok { "OK" } else { "BAD" }
        ));
    }
    let total = if bad == 0 { "OK".to_string() } else { format!("{bad} BAD") };
    lines.push(format!("| **total** | | {t_size:#x} | | {t_func:#x} | | {t_gap:#x} | {total} |"));
    lines.extend(["", "## Build artifacts vs ROM", ""].map(String::from));
    let mut checks = Vec::new();
    for region in REGIONS {
        let rom = &roms[region];
        let sbin = decomp.join(format!("build/pmdsky.{region}/main.sbin"));
        if sbin.exists() {
            let data = fs::read(&sbin)?;
            let start = rom.u32(0x20) as usize;
            let ok = rom.d.get(start..start + data.len()) == Some(&data[..]);
            checks.push(format!("{region} main.sbin: {} ({:#x} bytes)", if ok { "OK" } else { "MISMATCH" }, data.len()));
        }
        let overlays = rom.overlays();
        for i in 0..36u32 {
            let p = decomp.join(format!("build/pmdsky.{region}/OVY_{i}.sbin"));
            if !p.exists() {
                continue;
            }
            let data = fs::read(&p)?;
            let (start, end) = rom.fat_entry(overlays[&i].file_id);
            let ok = rom.d[start..end] == data[..];
            checks.push(format!("{region} OVY_{i}.sbin: {} ({:#x} bytes)", if ok { "OK" } else { "MISMATCH" }, data.len()));
        }
    }
    let arm7 = decomp.join("sub/build/arm7.sbin");
    if arm7.exists() {
        let data = fs::read(&arm7)?;
        let rom = &roms["us"];
        let start = rom.u32(0x30) as usize;
        let ok = rom.d.get(start..start + data.len()) == Some(&data[..]);
        checks.push(format!("shared arm7.sbin: {} ({:#x} bytes)", if ok { "OK" } else { "MISMATCH" }, data.len()));
    }
    lines.extend(checks.into_iter().map(|c| format!("- {c}")));
    lines.push(String::new());
    fs::write(out_root.join("COVERAGE.md"), lines.join("\n") + "\n")?;
    Ok(())
}

fn mode_intervals(rows: &[Row]) -> Vec<(u64, u64, char)> {
    let mut maps: Vec<(u64, &str)> =
        rows.iter().filter(|r| matches!(r.name.as_str(), "$a" | "$t" | "$d")).map(|r| (r.addr, r.name.as_str())).collect();
    maps.sort();
    (0..maps.len())
        .map(|i| (maps[i].0, maps.get(i + 1).map_or(1 << 32, |m| m.0), maps[i].1.chars().nth(1).unwrap()))
        .collect()
}

fn canonical(rel: &str) -> String {
    let p: Vec<&str> = rel.split('/').collect();
    if p.len() > 4 && p[2] == "DONE" {
        [&p[..2], &p[4..]].concat().join("/")
    } else {
        rel.to_string()
    }
}

pub fn verify(decomp: &Path, corpus: &Path) -> Result<()> {
    let arm9_mods = arm9_modules();
    let mut mods: HashMap<(String, String), (u64, Vec<u8>)> = HashMap::new();
    let mut modes: HashMap<(String, String), Vec<(u64, u64, char)>> = HashMap::new();
    let empty = Vec::new();
    for region in REGIONS {
        let rom = Rom::open(&decomp.join(format!("build/pmdsky.{region}/pmdsky.{region}.nds")))?;
        let xm = parse_xmap(&decomp.join(format!("build/pmdsky.{region}/main.nef.xMAP")))?;
        for m in &arm9_mods {
            mods.insert((region.into(), m.clone()), mod_bytes(&rom, m)?);
            modes.insert((region.into(), m.clone()), mode_intervals(xm.get(m).unwrap_or(&empty)));
        }
    }
    let rom = Rom::open(&decomp.join("build/pmdsky.us/pmdsky.us.nds"))?;
    let x7 = parse_xmap(&decomp.join("sub/build/arm7.nef.xMAP"))?;
    for (m, val) in arm7_mod_bytes(&rom)? {
        mods.insert(("shared".into(), m.into()), val);
        modes.insert(("shared".into(), m.into()), mode_intervals(x7.get(m).unwrap_or(&empty)));
    }
    let module_of_dir: HashMap<(String, String), String> = arm9_mods
        .iter()
        .map(|m| (("arm9".to_string(), dirname(m)), m.clone()))
        .chain(ARM7_MODULES.iter().map(|m| (("arm7".to_string(), dirname(m)), m.to_string())))
        .collect();

    let mut errors: Vec<String> = Vec::new();
    let mut files = BTreeSet::new();
    let mut index = HashSet::new();
    let index_text = fs::read_to_string(corpus.join("index.tsv")).context("reading index.tsv")?;
    for line in index_text.lines().skip(1) {
        let cells: Vec<&str> = line.split('\t').collect();
        ensure!(cells.len() == 7, "malformed index.tsv line: {line}");
        let (fam, module, name) = (cells[0], cells[1], cells[2]);
        index.insert((fam, module, name));
        let addr_cell = cells[3..6].iter().find(|c| **c != "-").context("index row with no address")?;
        let addr = u64::from_str_radix(addr_cell.split(':').next().unwrap().trim_start_matches("0x"), 16)?;
        files.insert(format!("{fam}/{}/{addr:08X}_{}.json", dirname(module), sanitize(name)));
    }

    let mut on_disk: BTreeMap<String, String> = BTreeMap::new();
    for fam in ["arm9", "arm7"] {
        let mut stack = vec![corpus.join(fam)];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if entry.file_type()?.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "json") {
                    let rel = path.strip_prefix(corpus).unwrap().to_string_lossy().replace('\\', "/");
                    let canon = canonical(&rel);
                    if let Some(prev) = on_disk.insert(canon.clone(), rel.clone()) {
                        errors.push(format!("duplicate {canon}: {prev} and {rel}"));
                    }
                }
            }
        }
    }
    let disk_keys: BTreeSet<String> = on_disk.keys().cloned().collect();
    for x in files.difference(&disk_keys).take(5) {
        errors.push(format!("not on disk: {x}"));
    }
    for x in disk_keys.difference(&files).take(5) {
        errors.push(format!("not in index: {x}"));
    }

    let mut func_bytes: HashMap<(String, String), u64> = HashMap::new();
    let mut checked = 0;
    for rel in files.intersection(&disk_keys) {
        let path = corpus.join(&on_disk[rel]);
        let rec: OutRecord = serde_json::from_slice(&fs::read(&path)?).with_context(|| format!("parsing {}", path.display()))?;
        let (fam, dir) = rec.module.split_once('/').context("bad module field")?;
        let module = module_of_dir.get(&(fam.to_string(), dir.to_string())).with_context(|| format!("{rel}: unknown module"))?;
        let key = |r: &str| if fam == "arm7" { "shared".to_string() } else { r.to_string() };
        let mut regions: Vec<(String, u64, u64)> = Vec::new();
        let mut items: HashMap<String, Vec<(u64, String, String)>> = HashMap::new();
        for (r, e) in rec.regions.iter() {
            if fam == "arm7" && e.same_as.is_some() {
                continue;
            }
            let parse = |s: &str| u64::from_str_radix(s.trim_start_matches("0x"), 16);
            regions.push((key(r), parse(&e.addr)?, parse(&e.size)?));
            if let Some(listing) = &e.listing {
                let v = items.entry(key(r)).or_default();
                for line in listing {
                    if let Some(c) = INSN.captures(line) {
                        v.push((u64::from_str_radix(&c[1], 16)?, c[2].replace(' ', ""), c[3].to_string()));
                    }
                }
            }
        }
        let Some(baseline) = regions.iter().find(|(r, _, _)| items.contains_key(r)).map(|(r, _, _)| r.clone()) else {
            errors.push(format!("{rel}: no listing"));
            continue;
        };
        let (_, baddr, bsize) = regions.iter().find(|(r, _, _)| *r == baseline).unwrap().clone();
        let (bbase, bmod) = &mods[&(baseline.clone(), module.clone())];
        let bslice = slice_clamped(bmod, baddr - bbase, bsize).to_vec();
        for (region, addr, size) in &regions {
            let (base, moddata) = &mods[&(region.clone(), module.clone())];
            let rom_slice = slice_clamped(moddata, addr - base, *size);
            if let Some(seq) = items.get(region) {
                let mut buf = Vec::new();
                let mut first = None;
                for (a, hex, mnemonic) in seq {
                    let first = *first.get_or_insert(*a);
                    ensure!(*a == first + buf.len() as u64, "{rel}: {region} non-contiguous at {a:#x}");
                    let mut p: Vec<u8> = (0..hex.len() / 2).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16)).collect::<Result<_, _>>()?;
                    let kind = modes[&(region.clone(), module.clone())]
                        .iter()
                        .find(|(s, e, _)| *s <= *a && *a < *e)
                        .map_or('a', |(_, _, k)| *k);
                    if kind == 't' && !mnemonic.starts_with('.') && p.len() == 4 {
                        p = vec![p[1], p[0], p[3], p[2]];
                    } else {
                        p.reverse();
                    }
                    buf.extend(p);
                }
                if first != Some(*addr) {
                    errors.push(format!("{rel}: {region} first addr {:#x} != {addr:#x}", first.unwrap_or(0)));
                }
                if buf.len() as u64 != *size {
                    errors.push(format!("{rel}: {region} got {:#x} bytes want {size:#x}", buf.len()));
                } else if buf != rom_slice {
                    errors.push(format!("{rel}: {region} byte mismatch vs ROM"));
                }
            } else if *size != bsize || rom_slice != &bslice[..] {
                errors.push(format!("{rel}: {region} claimed identical but ROM bytes differ"));
            }
            *func_bytes.entry((region.clone(), module.clone())).or_default() += size;
        }
        checked += 1;
    }

    println!("files checked: {checked}");
    println!("index rows: {}", index.len());
    println!("errors: {}", errors.len());
    for e in errors.iter().take(20) {
        println!("  {e}");
    }
    let (mut tot_mod, mut tot_func, mut neg) = (0u64, 0u64, 0);
    let mut keys: Vec<&(String, String)> = mods.keys().collect();
    keys.sort();
    for k in keys {
        let len = mods[k].1.len() as u64;
        let fb = func_bytes.get(k).copied().unwrap_or(0);
        if fb > len {
            neg += 1;
            println!("  NEGATIVE GAP {} {}: func {fb:#x} > module {len:#x}", k.0, k.1);
        }
        tot_mod += len;
        tot_func += fb;
    }
    println!("module bytes {tot_mod:#x}  function bytes {tot_func:#x}  gap bytes {:#x}  neg {neg}", tot_mod - tot_func);
    if !errors.is_empty() || neg > 0 {
        bail!("corpus verification failed");
    }
    Ok(())
}
