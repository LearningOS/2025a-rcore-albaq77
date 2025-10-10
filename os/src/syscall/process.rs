//! Process management syscalls
use crate::config::PAGE_SIZE;
use crate::task::{change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, current_user_token, TASK_MANAGER};
use crate::mm::{translated_byte_buffer, MapPermission, PageTable, VirtAddr};
use crate::timer::get_time_us;


#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");

    let us = get_time_us();
    let time_val = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    let buffers = translated_byte_buffer(
        current_user_token(), 
        _ts as *const u8,
        core::mem::size_of::<TimeVal>());

    let mut time_val_ptr: *const u8;
    
    time_val_ptr = (&time_val as *const TimeVal).cast::<u8>();
    
    for buffer in buffers{
        unsafe {
            time_val_ptr.copy_to(buffer.as_mut_ptr(), buffer.len());
            time_val_ptr = time_val_ptr.add(buffer.len());
        }
    }
    0
}

/// Read a byte from user space virtual address
fn read_user_byte(token: usize, va: usize) -> Option<u8> {

    if va == 0 || va >= 0x80200000 {
        return None;
    }
    let page_table = PageTable::from_token(token);
    let va = VirtAddr::from(va);
    let vpn = va.floor();
    
    let pte = page_table.translate(vpn)?;
    
    if !pte.is_valid() || !pte.readable() {
        return None;
    }
    
    let ppn = pte.ppn();
    let offset = va.page_offset();
    let byte_array = ppn.get_bytes_array();
    Some(byte_array[offset])
}

/// Write a byte to user space virtual address
fn write_user_byte(token: usize, va: usize, data: u8) -> bool {

    if va == 0 || va >= 0x80200000 {
        return false;
    }
    let page_table = PageTable::from_token(token);
    let va = VirtAddr::from(va);
    let vpn = va.floor();
    let pte = page_table.translate(vpn);
    if pte.is_none() {
        return false;
    }
    let pte = pte.unwrap();
    if !pte.is_valid() || !pte.writable() {
        return false;
    }
    let ppn = pte.ppn();
    let offset = va.page_offset();
    let byte_array = ppn.get_bytes_array();
    byte_array[offset] = data;
    true
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(_trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    match _trace_request {
        0 => {
            if let Some(byte) = read_user_byte(current_user_token(), id) {
                byte as isize
            } else {
                -1
            }
        }
        1 => {
            if write_user_byte(current_user_token(), id, data as u8) {
                0
            } else {
                -1
            }
        }
        2 => {
            TASK_MANAGER.get_task_syscall_count(id)
        }
        _ => -1,
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap start={:#x}, len={:#x}, port={:#x}", start, len, port);
    if start % PAGE_SIZE != 0 {
        return -1;
    }
    if port & !0x7 != 0 || port & 0x7 == 0 {
        return -1;
    }
    if len == 0 {
        return 0;
    }

    let mut map_perm = MapPermission::U;

    if port & 0x1 != 0 {
        map_perm |= MapPermission::R;
    }
    if port & 0x2 != 0 {
        map_perm |= MapPermission::W;
    }

    if port & 0x4 != 0 {
        map_perm |= MapPermission::X;
    }
    let start_va: VirtAddr = VirtAddr::from(start);
    let end_va: VirtAddr = VirtAddr::from(start + len);
    let start_vpn = start_va.floor();
    let end_vpn = end_va.ceil();
    let pass = TASK_MANAGER.task_mmap(start_va, end_va, start_vpn, end_vpn, map_perm);
    if pass {
        return 0; 
    }
    -1
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap start={:#x}, len={:#x}", start, len);
    if start % PAGE_SIZE != 0 {
        return -1;
    }
    if len == 0 {
        return 0;
    }

    let start_va: VirtAddr = VirtAddr::from(start);
    let end_va: VirtAddr = VirtAddr::from(start + len);
    let start_vpn= start_va.floor();
    let end_vpn = end_va.ceil();
    let pass = TASK_MANAGER.task_munmap(start_vpn, end_vpn);
    if pass {
        return 0;
    }
    -1
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
