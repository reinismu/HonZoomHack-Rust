mod linux_process;
use failure::{bail, Error};
use goblin::elf::sym::Sym;
use proc_maps::Pid;
use std::collections::HashMap;

const ZOOM_OUT: &str = "_ZN7CPlayer7ZoomOutEv";
const PREPARE_CLIENT_STATE: &str = "_ZN7CPlayer18PrepareClientStateER12CClientStateb";
const SETUP_CAMERA: &str = "_ZN7CPlayer11SetupCameraER7CCameraRK5CVec3IfES5_";

fn zoom_hack() -> Result<(), Error> {
    let process_name = "hon-x86_64";
    let p_id = linux_process::get_process_id_by_name(process_name.to_string())?;

    println!("Found process_id for {}: {}", process_name, p_id);
    let map_range = linux_process::get_process_map_range(p_id, |map| {
        map.filename()
            .clone()
            .unwrap_or_else(|| "".to_string())
            .contains("shared")
    })?;
    let filename = map_range.filename().clone().unwrap();

    let symbol_map = linux_process::get_symbol_map(&filename)?;
    println!("ZOOM_OUT {:?}", symbol_map.get(ZOOM_OUT));

    linux_process::attach(p_id)?;
    linux_process::waitpid(p_id)?;

    println!("Attached");
    let result = patch(p_id, map_range.start(), &symbol_map);

    match result {
        Ok(_) => {
            linux_process::detach(p_id)?;
            println!("Patched");
        }
        Err(e) => {
            linux_process::detach(p_id)?;
            bail!("Error: {:?}", e);
        }
    }

    Ok(())
}

fn patch(pid: Pid, module_offset: usize, symbol_map: &HashMap<String, Sym>) -> Result<(), Error> {
    let zoom_out_offset = symbol_map.get(ZOOM_OUT).unwrap().st_value as usize;
    let prepare_client_state_offset =
        symbol_map.get(PREPARE_CLIENT_STATE).unwrap().st_value as usize;
    let setup_camera_offset = symbol_map.get(SETUP_CAMERA).unwrap().st_value as usize;

    // ZoomOut + 0x4f (minss   xmm1, xmm2) -> (movss   xmm1, xmm2)
    linux_process::write_process_memory(
        pid,
        zoom_out_offset + module_offset + 0x4f,
        vec![0xF3, 0x0F, 0x10, 0xCA],
    )?;

    // ZoomOut + 0x5e (minss   xmm1, xmm3) -> (movss   xmm1, xmm3)
    linux_process::write_process_memory(
        pid,
        zoom_out_offset + module_offset + 0x5e,
        vec![0xF3, 0x0F, 0x10, 0xCB],
    )?;

    // PrepareClientState + 0x222 (movss   cs:currentCammeraZoom, xmm0) -> (NOP, NOP ..)
    linux_process::write_process_memory(
        pid,
        prepare_client_state_offset + module_offset + 0x222,
        vec![0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90],
    )?;

    //PrepareClientState + 0xc78 (minss   xmm2, xmm3 ) -> (movss   xmm2, xmm3)
    linux_process::write_process_memory(
        pid,
        prepare_client_state_offset + module_offset + 0xc78,
        vec![0xF3, 0x0F, 0x10, 0xD3],
    )?;

    //PrepareClientState + 0xc91 (ja      short loc_C9847C ) -> (jmp      short loc_C9847C)
    linux_process::write_process_memory(
        pid,
        prepare_client_state_offset + module_offset + 0xc91,
        vec![0x90, 0x90],
    )?;

    //PrepareClientState + 0xc93 (minss   xmm1, xmm2) -> movss   xmm1, xmm2)
    linux_process::write_process_memory(
        pid,
        prepare_client_state_offset + module_offset + 0xc93,
        vec![0xF3, 0x0F, 0x10, 0xCA],
    )?;

    //SetupCamera + 0x5df (jbe     short loc_C97702 ) -> (jmp     short loc_C97702)
    linux_process::write_process_memory(
        pid,
        setup_camera_offset + module_offset + 0x5df,
        vec![0xEB],
    )?;

    Ok(())
}

fn main() {
    match zoom_hack() {
        Ok(_) => println!("Succeeded!"),
        Err(e) => println!("Failed {:?}", e),
    }
}
