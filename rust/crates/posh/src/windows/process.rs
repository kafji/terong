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

// adapted from https://learn.microsoft.com/en-us/windows/win32/psapi/enumerating-all-processes

pub fn processes() -> Vec<ProcessInfo> {
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
        processes
            .into_iter()
            .take(count)
            .filter_map(get_process_info)
            .collect()
    }
}

pub fn get_process_info(pid: u32) -> Option<ProcessInfo> {
    unsafe {
        OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid)
            .ok()
            .map(|process| {
                let module = {
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
                    module
                };
                let base_name = {
                    let mut buf = [0; MAX_PATH as usize];
                    let len = GetModuleBaseNameW(process, module.into(), &mut buf);
                    String::from_utf16(&buf[..len as usize]).unwrap()
                };
                CloseHandle(process).unwrap();
                ProcessInfo {
                    pid,
                    cmd: base_name,
                }
            })
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct ProcessInfo {
    pub pid: u32,
    pub cmd: String,
}
