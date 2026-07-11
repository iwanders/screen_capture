#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

pub type key_t = i32;
pub const IPC_PRIVATE: key_t = 0;
pub const IPC_CREAT: i32 = 0x200; /* create if key is nonexistent */
pub const IPC_RMID: i32 = 0; /* remove resource, from linux/ipc.h */

unsafe extern "C" {
    pub fn shmget(key: key_t, size: u64, shmflg: i32) -> i32;
    pub fn shmat(shmid: i32, shmaddr: *const libc::c_void, shmflg: i32) -> *mut libc::c_void;
    pub fn shmdt(shmaddr: *const libc::c_void) -> i32;
    pub fn shmctl(shmid: i32, cmd: i32, shmids_buf: *const libc::c_void) -> i32;
}
