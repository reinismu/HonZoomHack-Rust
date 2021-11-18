use failure::{bail, Error};
use goblin::elf::sym::Sym;
use nix::sys::ptrace::AddressType;
use proc_maps::{get_process_maps, MapRange, Pid};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::os::raw::c_void;

pub fn get_process_id_by_name(name: String) -> Result<Pid, Error> {
    let entries = fs::read_dir("/proc")?;

    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let mut comm_path = entry.path();
        comm_path.push("comm");

        if !comm_path.exists() {
            continue;
        }

        let output = fs::read(comm_path.as_path())?;
        let output = String::from_utf8(output)?;

        if output.replace("\n", "") == name {
            let pid = match entry.file_name().into_string() {
                Ok(pid) => pid,
                Err(e) => bail!("Cannot parse entry: {:?}", e),
            };
            let pid = pid.parse::<Pid>()?;
            return Ok(pid);
        }
    }
    bail!("Unable to find process named: {}", name)
}

pub fn get_process_map_range(pid: Pid, pred: fn(&MapRange) -> bool) -> Result<MapRange, Error> {
    let maps = get_process_maps(pid)?;
    for map in maps {
        if pred(&map) && map.is_exec() {
            println!(
                "Found module: Filename {:?} Address {} Size {} - {}",
                map.filename(),
                map.start(),
                map.size(),
                map.is_exec(),
            );
            return Ok(map);
        }
    }
    bail!("Unable to find symbols for process with id: {}", pid)
}

pub fn get_symbol_map(binary_path: &String) -> Result<HashMap<String, Sym>, Error> {
    let mut f = File::open(binary_path)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).unwrap();
    let bytes = buf.as_slice();

    Ok(get_symbol_map_from_bytes(bytes))
}

pub fn attach(pid: Pid) -> Result<(), Error> {
    use nix::sys::ptrace::attach;
    let pid = nix::unistd::Pid::from_raw(pid);
    attach(pid)?;

    Ok(())
}

pub fn detach(pid: Pid) -> Result<(), Error> {
    use nix::sys::ptrace::detach;
    let pid = nix::unistd::Pid::from_raw(pid);
    detach(pid)?;

    Ok(())
}

pub fn waitpid(pid: Pid) -> Result<(), Error> {
    use nix::sys::wait::waitpid;
    let pid = nix::unistd::Pid::from_raw(pid);
    waitpid(pid, None)?;

    Ok(())
}

pub fn write_process_memory(pid: Pid, addr: usize, buffer: Vec<u8>) -> Result<(), Error> {
    use byteorder::{ByteOrder, LittleEndian};
    use nix::sys::ptrace::read;
    use nix::sys::ptrace::write;

    let pid = nix::unistd::Pid::from_raw(pid);

    let pointer_size = std::mem::size_of::<AddressType>();
    let mut address = addr as AddressType;

    for chunk in buffer.chunks(pointer_size) {
        if chunk.len() == pointer_size {
            let data = LittleEndian::read_uint(chunk, pointer_size);
            write(pid, address, data as *mut c_void)?;
            println!("{:x?} Wrote: {:x?}", address, data);
        } else {
            let cur_data = read(pid, address)?;
            let slice = unsafe { any_as_u8_slice(&cur_data) };
            println!("{:x?} read in data: {:x?}", address, slice);
            for (i, b) in chunk.iter().enumerate() {
                slice[i] = *b;
            }

            let data = LittleEndian::read_uint(slice, pointer_size);

            write(pid, address, data as *mut c_void)?;
            println!("{:x?} Wrote at end: {:x?}", address, data);
        }
        address = (address as usize + pointer_size) as AddressType;
    }

    Ok(())
}

unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &mut [u8] {
    ::std::slice::from_raw_parts_mut((p as *const T) as *mut u8, ::std::mem::size_of::<T>())
}

fn get_symbol_map_from_bytes(bytes: &[u8]) -> HashMap<String, Sym> {
    let mut symbol_map = HashMap::new();

    let parsed_binary = goblin::elf::Elf::parse(&bytes);
    if let Ok(binary) = parsed_binary {
        let elf = binary as goblin::elf::Elf;
        let syms = elf.dynsyms.to_vec();
        let strtab = elf.dynstrtab;
        for sym in syms {
            if let Some(Ok(symbol_name)) = strtab.get(sym.st_name) {
                symbol_map.insert(symbol_name.to_string(), sym);
            }
        }
    } else {
        println!("Failed parsing elf {:?}", parsed_binary);
    }
    symbol_map
}
