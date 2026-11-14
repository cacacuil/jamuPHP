//! Bagian yang khusus Windows: Job Object, Ctrl-Break, dan path.

use crate::supervise::Stop;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::{env, ffi::c_void};
use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

pub const BIN_DIRS: &[&str] = &[];

pub fn is_root() -> bool {
    false
}

/// `canonicalize` di Windows menghasilkan `\\?\C:\...`. Caddy dan PHP tidak mengerti
/// awalan itu, jadi dibuang untuk path drive biasa.
pub fn tidy(p: PathBuf) -> PathBuf {
    match p.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => p,
    }
}

pub fn state_base() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("jamuphp")
}

pub fn fallback_state_base() -> Option<PathBuf> {
    None
}

/// Supervisor masuk ke Job Object dengan KILL_ON_JOB_CLOSE, dan setiap proses yang
/// dinyalakannya ikut di dalamnya. Saat supervisor mati, dengan cara apa pun, Windows
/// menutup handle job dan mematikan semua yang tersisa. Ini padanan PR_SET_PDEATHSIG.
pub fn init() {
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let size = std::mem::size_of_val(&info) as u32;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const c_void,
            size,
        );
        // Handle sengaja tidak ditutup: ia harus hidup selama supervisor hidup.
        AssignProcessToJobObject(job, GetCurrentProcess());
    }
}

pub fn prepare(cmd: &mut Command) {
    // CREATE_NEW_PROCESS_GROUP: Ctrl-C di konsol hanya sampai ke supervisor, dan
    // Ctrl-Break bisa dikirim ke satu anak saja.
    cmd.creation_flags(0x0000_0200);
}

/// Windows tidak punya sinyal. Caddy mengartikan Ctrl-Break sebagai permintaan berhenti
/// dengan rapi; php-cgi langsung dimatikan, dan Job Object miliknya ikut mematikan pekerjanya.
pub fn ask_to_stop(child: &mut Child, how: Stop) {
    let asked = matches!(how, Stop::Term)
        && unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, child.id()) } != 0;
    if !asked {
        let _ = child.kill();
    }
}

pub fn kill_tree(child: &mut Child) {
    let _ = child.kill();
}

pub fn reap_orphans(_ours: &[u32]) {}

pub fn unwritable(_dir: &Path) -> Option<String> {
    None
}

pub fn port_holder(_port: u16) -> String {
    String::new()
}
