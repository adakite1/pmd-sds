use std::path::Path;

use anyhow::{Context, Result};
use object::{Endianness, Object, ObjectSegment, elf::{FileHeader32, PT_LOAD}, read::elf::{FileHeader, ProgramHeader}};

mod corpus;
mod corpus_gen;
mod corpus_layout;

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

/// Turns an ELF binary to a flat memory image, along with the base and ELF entrypoint addresses.
fn flatten2(path: &Path, skip_bss: bool, use_paddr: bool) -> Result<(u32, u32, Vec<u8>)> {
    let file = std::fs::read(path)?;
    let data = file.as_slice();
    let header = FileHeader32::<Endianness>::parse(data)?; // checks the ELF magic, that the ELF is of ELFCLASS32, and the version
    let endian = header.endian()?; // runtime endianness from e_ident

    let mut segs: Vec<(u64, &[u8], u64)> = header
        .program_headers(endian, data)? // program header segments (what maps into memory)
        .iter()
        .filter(|ph| ph.p_type(endian) == PT_LOAD) // loadable segments only (we need to filter explicitly here unlike with the unified api version above)
        .filter_map(|ph| {
            let d = ph.data(endian, data).ok()?; // file-backed bytes go [p_offset, p_offset + p_filesz)
            let memsz = if skip_bss { d.len() as u64 } else { u64::from(ph.p_memsz(endian)) }; // the in-memory size
            (memsz > 0).then(|| (u64::from(if use_paddr { ph.p_paddr(endian) } else { ph.p_vaddr(endian) }), d, memsz)) // finds all valid (virtual_address, data_slice, memsz) to map
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

    Ok((base as u32, header.e_entry(endian), out))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("corpus") => corpus::main(&args[1..]),
        _ => {
            Ok(())
        }
    }
}
