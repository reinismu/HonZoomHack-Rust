mod linux_process {
    use std::fs;

    pub fn get_process_id_by_name(name: String) -> Option<u32> {
        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Ok(file_type) = entry.file_type() {
                        if !file_type.is_dir() {
                            continue;
                        }
                    }
                    let mut comm_path = entry.path();
                    comm_path.push("comm");

                    let output = fs::read(comm_path.as_path());
                    if let Ok(output) = output {
                        if let Ok(output) = String::from_utf8(output) {
                            if output.replace("\n", "") == name {
                                if let Ok(process_id) = entry.file_name().into_string() {
                                    return Some(process_id.parse::<u32>().unwrap());
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

fn main() {
    let process_name = "hon";
    let process_id = linux_process::get_process_id_by_name(process_name.to_string());

    if let Some(ref p_id) = process_id {
        println!("Found process_id for {}: {}", process_name, p_id);
    } else {
        println!("Failed to find process_id for {}", process_name);
    }
}
