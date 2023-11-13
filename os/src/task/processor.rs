//!Implementation of [`Processor`] and Intersection of control flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::config::MAX_SYSCALL_NUM;
use crate::mm::{VirtAddr, MapPermission};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;

/// Processor management structure
pub struct Processor {
    ///The task currently executing on the current processor
    current: Option<Arc<TaskControlBlock>>,

    ///The basic control flow of each core, helping to select and switch process
    idle_task_cx: TaskContext,
}

impl Processor {
    ///Create an empty Processor
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    ///Get mutable reference to `idle_task_cx`
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }

    ///Get current task in moving semanteme
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    ///Get current task in cloning semanteme
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }
}

lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

///The main part of process execution and scheduling
///Loop `fetch_task` to get the process that needs to run, and switch the process through `__switch`
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            // release coming task_inner manually
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            warn!("no tasks available in run_tasks");
        }
    }
}

/// Get current task through take, leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// Get a copy of the current task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// Get the current user token(addr of page table)
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.get_user_token()
}

///Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

///Return to idle control flow for new scheduling
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}


/// Get the first run time of current `Running` task.
pub fn get_current_task_time() -> usize {
    // TASK_MANAGER.get_current_time();
    current_task().unwrap().inner_exclusive_access().task_time
}

/// Get all system call counts of current `Running` task.
pub fn get_current_syscall_times() -> [u32; MAX_SYSCALL_NUM] {
    // TASK_MANAGER.get_current_syscall_times()
    current_task().unwrap().inner_exclusive_access().task_syscall_times
}

/// Update the specific system call count of current `Running` task.
pub fn update_current_syscall_times(syscall_id: usize) {
    // TASK_MANAGER.update_current_syscall_times(syscall_id);
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .task_syscall_times[syscall_id] += 1;
}

/// Insert a new area into the current `Running` task's memory set.
pub fn task_insert_framed_area(start_va: VirtAddr, end_va: VirtAddr, permission: MapPermission) {
    // TASK_MANAGER.task_insert_framed_area(start_va, end_va, permission);
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .memory_set
        .insert_framed_area(start_va, end_va, permission);
}

/// Drop an area from the current `Running` task's memory set.
pub fn task_drop_framed_area(start_va: VirtAddr, end_va: VirtAddr) -> isize{
    // TASK_MANAGER.task_drop_framed_area(start_va, end_va)
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .memory_set
        .drop_framed_area(start_va, end_va)
}

// / Set the priority of current `Running` task.
// pub fn set_priority(priority: usize) {
//     current_task()
//         .unwrap()
//         .inner_exclusive_access()
//         .priority = priority;
// }
//
// it does not work, why