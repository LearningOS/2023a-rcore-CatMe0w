//! Process management syscalls
use crate::{
    config::{MAX_SYSCALL_NUM, PAGE_SIZE},
    mm::{translated_pa, VirtAddr, PageTable, StepByOne, MapPermission},
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus, current_user_token, get_current_syscall_times, get_current_task_time, task_insert_framed_area, task_drop_framed_area,
    }, timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
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
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    // let ts = translated_pa(current_user_token(), ts as usize) as *mut TimeVal;
    let ts = translated_pa(current_user_token(), ts as usize) as &mut TimeVal;
    // unsafe {
        // *ts = TimeVal {
        //     sec: us / 1_000_000,
        //     usec: us % 1_000_000,
        // };
    // }
    ts.sec = us / 1_000_000;
    ts.usec = us % 1_000_000;
    0
    
    // note: alright, the ch3_sleep just can be (always) passed on my computer after I left ch3 with nothing changed...
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info");
    let ti = translated_pa(current_user_token(), _ti as usize) as &mut TaskInfo;
    // unsafe {
    //     (*ti).status = TaskStatus::Running;
    //     (*ti).syscall_times = get_current_syscall_times();
    //     (*ti).time = get_current_task_time();
    // }
    ti.status = TaskStatus::Running;
    ti.syscall_times = get_current_syscall_times();
    ti.time = get_current_task_time();
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap");
    let start_va = VirtAddr::from(start);
    if port & !0x7 != 0 {
        return -1;
    }
    if port & 0x7 == 0 {
        return -1;
    }
    if start_va.page_offset() != 0 {
        return -1;
    }
    let mut vpn_start = start_va.floor();
    let mut end_va = VirtAddr::from(start + len - 1);
    let pt = PageTable::from_token(current_user_token());
    let required_pages = (len + PAGE_SIZE - 1) / PAGE_SIZE;
    for _ in 0..required_pages {
        // let pte = page_table.translate(vpn_start + i).unwrap();
        let pte = pt.translate(vpn_start);
        if pte.is_some() {
            if pte.unwrap().is_valid() {
                return -1;
            }
        }
        vpn_start.step();
    }
    if end_va.page_offset() == 0 {
        // va_end.step();
        end_va = VirtAddr::from(end_va.0 + 1);
    }
    // let permission = MapPermission::from_bits_truncate(port & 0x7);
    let mut permission = MapPermission::empty();
    permission.set(MapPermission::R, port & 0x1 != 0);
    permission.set(MapPermission::W, port & 0x2 != 0);
    permission.set(MapPermission::X, port & 0x4 != 0);
    permission.set(MapPermission::U, true);
    task_insert_framed_area(start_va, end_va, permission);
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");
    let start_va = VirtAddr::from(start);
    let mut end_va = VirtAddr::from(start + len - 1);
    if start_va.page_offset() != 0 {
        return -1;
    }
    if end_va.page_offset() == 0 {
        // va_end.step();
        end_va = VirtAddr::from(end_va.0 + 1);
    }
    task_drop_framed_area(start_va, end_va)

    // my head hurts
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
