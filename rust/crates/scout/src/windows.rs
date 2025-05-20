//! References:
//! - https://learn.microsoft.com/en-us/windows/win32/fileio/walking-a-buffer-of-change-journal-records

use std::{
    mem::{size_of, size_of_val},
    slice,
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE},
        Storage::FileSystem::{
            CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
            GetLogicalDriveStringsW, OPEN_EXISTING,
        },
        System::{
            IO::DeviceIoControl,
            Ioctl::{
                FSCTL_QUERY_USN_JOURNAL, FSCTL_READ_USN_JOURNAL, READ_USN_JOURNAL_DATA_V0,
                USN_JOURNAL_DATA_V0, USN_REASON_FILE_CREATE, USN_REASON_FILE_DELETE,
                USN_REASON_RENAME_NEW_NAME, USN_REASON_RENAME_OLD_NAME, USN_RECORD_V2,
            },
        },
    },
    core::HSTRING,
};

pub fn run() {
    unsafe {
        let mut buf = [0; 1024];
        let n = GetLogicalDriveStringsW(Some(&mut buf));
        let n = n as _;
        assert!(buf.len() > n);
        let drives: Vec<_> = buf[..n]
            .split(|&c| c == 0)
            .filter(|&x| !x.is_empty())
            .map(|x| String::from_utf16_lossy(x))
            .collect();

        for drive in drives {
            // https://learn.microsoft.com/en-us/windows/win32/fileio/obtaining-a-volume-handle-for-change-journal-operations
            let letter = drive.split(':').next().unwrap();
            let drive = format!(r#"\\.\{}:"#, letter);
            let drive = HSTRING::from(drive);
            dbg!(&drive);

            let handle = CreateFileW(
                &drive,
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            );
            match handle {
                Ok(handle) => {
                    // https://learn.microsoft.com/en-us/windows/win32/api/WinIoCtl/ns-winioctl-usn_journal_data_v0
                    let mut journal_data = USN_JOURNAL_DATA_V0::default();
                    let control = DeviceIoControl(
                        handle,
                        FSCTL_QUERY_USN_JOURNAL,
                        None,
                        0,
                        Some(&mut journal_data as *mut _ as *mut _),
                        size_of_val(&journal_data) as _,
                        None,
                        None,
                    );
                    if let Err(err) = control {
                        eprintln!("failed to query usn journal: {}", err);
                        continue;
                    }

                    // https://learn.microsoft.com/en-us/windows/win32/api/WinIoCtl/ns-winioctl-read_usn_journal_data_v0
                    let mut read_data = READ_USN_JOURNAL_DATA_V0 {
                        StartUsn: 0,
                        ReasonMask: USN_REASON_FILE_CREATE
                            | USN_REASON_FILE_DELETE
                            | USN_REASON_RENAME_NEW_NAME
                            | USN_REASON_RENAME_OLD_NAME,
                        ReturnOnlyOnClose: 0,
                        Timeout: 0,
                        BytesToWaitFor: 0,
                        UsnJournalID: journal_data.UsnJournalID,
                    };
                    for _ in 0..2 {
                        dbg!(&read_data.StartUsn);

                        let mut buf = [0u8; 4096];
                        let mut write = 0;
                        let control = DeviceIoControl(
                            handle,
                            FSCTL_READ_USN_JOURNAL,
                            Some(&read_data as *const _ as *const _),
                            size_of_val(&read_data) as _,
                            Some(buf.as_mut_ptr() as _),
                            size_of_val(&buf) as _,
                            Some(&mut write),
                            None,
                        );
                        if let Err(err) = control {
                            eprintln!("failed to read usn journal: {}", err);
                            break;
                        }
                        dbg!(write);
                        let write = write as _;
                        let buf = &buf[..write];

                        let mut read = size_of::<i64>();
                        while read < write {
                            let record_ptr: *const USN_RECORD_V2 = buf.as_ptr().byte_add(read) as _;

                            let (record, length) = UsnRecord::from_v2_ptr(record_ptr);
                            dbg!(&record);

                            read += length;

                            read_data.StartUsn = record.usn;
                        }

                        // let next_usn = *(&raw const buf as *const _);
                        // read_data.StartUsn = next_usn;
                    }

                    if let Err(err) = CloseHandle(handle) {
                        eprintln!("failed to close handle: {}", err);
                    }
                }
                Err(err) => eprintln!("failed to open handle: {}", err),
            };
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
struct UsnRecord {
    usn: i64,
    timestamp: u64,
    reason: u32,
    file_name: String,
}

impl UsnRecord {
    /// References:
    /// - https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-usn_record_v2
    unsafe fn from_v2_ptr(ptr: *const USN_RECORD_V2) -> (Self, usize) {
        let record: &USN_RECORD_V2 = unsafe { &*ptr };
        // dbg!(record);

        assert_eq!(record.MajorVersion, 2);

        let usn = record.Usn;

        let timestamp = record.TimeStamp as _;

        let reason = record.Reason;

        let file_name_ptr = unsafe { (ptr as *const u16).byte_add(record.FileNameOffset as _) };
        let file_name = unsafe {
            slice::from_raw_parts::<u16>(file_name_ptr, (record.FileNameLength / 2) as usize)
        };
        let file_name = String::from_utf16_lossy(file_name);

        (
            Self {
                usn,
                timestamp,
                reason,
                file_name,
            },
            record.RecordLength as _,
        )
    }
}
