//! Process management syscalls
use alloc::sync::Arc;

use crate::{
    config::{MAX_SYSCALL_NUM, PAGE_SIZE},
    mm::{translated_pa, VirtAddr, PageTable, StepByOne, MapPermission, translated_refmut, translated_str},
    loader::get_app_data_by_name,
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next, TaskStatus, get_current_syscall_times, get_current_task_time, task_insert_framed_area, task_drop_framed_area,
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
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel:pid[{}] sys_yield", current_task().unwrap().pid.0);
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    trace!("kernel::pid[{}] sys_waitpid [{}]", current_task().unwrap().pid.0, pid);
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
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
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(path: *const u8) -> isize {
    let current_task = current_task().unwrap();
    trace!(
        "kernel:pid[{}] sys_spawn",
        current_task.pid.0
    );
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    let path = translated_str(new_task.get_user_token(), path);
    if let Some(d) = get_app_data_by_name(path.as_str()) {
        new_task.exec(d);
    } else {
        panic!("no such file");
    }
    add_task(new_task);
    new_pid as isize

    // mother may i?
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(prio: isize) -> isize {
    let current_task = current_task().unwrap();
    trace!(
        "kernel:pid[{}] sys_set_priority",
        current_task.pid.0
    );
    if prio >= 2 {
        current_task.set_priority(prio);
        prio
    } else {
        -1
    }
}
