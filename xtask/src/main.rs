use std::path::Path;

use anyhow::{Context, Result};
use object::{Object, ObjectSegment};

/// Turns an ELF binary to a flat memory image, along with the base and ELF entrypoint addresses.
fn flatten(path: &Path, skip_bss: bool) -> Result<(u32, u32, Vec<u8>)> {
    let data = std::fs::read(path)?;
    let elf = object::File::parse(&*data)?;

    let mut segs: Vec<(u64, &[u8], u64)> = elf
        .segments()  // program header segments (what maps into memory) ("segments" filters for loadable-segments-only automatically)
        .filter_map(|s| {
            let d = s.data().ok()?;
            let memsz = if skip_bss { d.len() as u64 } else { s.size() }; // the in-memory size
            (memsz > 0).then(|| (s.address(), d, memsz)) // finds all valid (virtual_address, data_slice, memsz) to map
        })
        .collect();
    segs.sort_by_key(|(a, _, _)| *a); // low addr to high

    let base = segs.first().context("no loadable segments at all")?.0;
    let end = segs.iter().map(|(a, _, m)| a + m).max().unwrap(); // there's at least one segment, so this is safe
    let mut out = vec![0u8; (end - base) as usize];

    for (addr, d, memsz) in segs {
        anyhow::ensure!(memsz >= d.len() as u64, "malformed segment where memsz < filesz, addr {:#010x} with memsz={} but filesz={} bytes", addr, memsz, d.len());
        let offset = (addr - base) as usize;
        out[offset..offset + d.len()].copy_from_slice(d);
    }

    Ok((base as u32, elf.entry() as u32, out))
}

fn main() {
    println!("Hello, world!");
}
