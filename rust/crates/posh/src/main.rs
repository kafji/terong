use std::mem;
use windows::Win32::{
    Foundation::{CloseHandle, HMODULE, MAX_PATH},
    System::{
        ProcessStatus::{
            EnumProcessModulesEx, EnumProcesses, GetModuleBaseNameW, LIST_MODULES_DEFAULT,
        },
        Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
    },
};

// https://learn.microsoft.com/en-us/windows/win32/psapi/enumerating-all-processes

fn main() {
    unsafe {
        let mut processes = [0; 1024];
        let mut needed = 0;
        EnumProcesses(
            (&mut processes).as_mut_ptr(),
            mem::size_of_val(&processes).try_into().unwrap(),
            &mut needed,
        )
        .unwrap();
        let count = needed as usize / mem::size_of::<u32>();
        for i in 0..(count.min(processes.len())) {
            let id = processes[i];
            print_process_name_and_id(id);
        }
    }
}

fn print_process_name_and_id(process_id: u32) {
    unsafe {
        let process = OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            process_id,
        );
        if let Ok(process) = process {
            let mut module = HMODULE::default();
            let mut needed = 0;
            EnumProcessModulesEx(
                process,
                &mut module,
                mem::size_of_val(&module).try_into().unwrap(),
                &mut needed,
                LIST_MODULES_DEFAULT,
            )
            .unwrap();
            let mut base_name = [0; MAX_PATH as usize];
            let length = GetModuleBaseNameW(process, module.into(), &mut base_name);
            CloseHandle(process).unwrap();

            let base_name = String::from_utf16(&base_name[..length as usize]).unwrap();
            println!("{} (PID: {})", base_name, process_id);
        }
    }
}
