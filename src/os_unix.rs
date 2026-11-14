//! Bagian yang khusus Unix: sinyal, grup proses, folder state, dan PID 1 di container.

use crate::supervise::Stop;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::{env, fs};

/// Folder tambahan untuk mencari binary: php-fpm dari paket distro ada di /usr/sbin,
/// yang sering tidak ada di PATH pengguna biasa.
pub const BIN_DIRS: &[&str] = &["/usr/sbin", "/usr/local/sbin", "/opt/jamuphp/bin"];

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub fn tidy(p: PathBuf) -> PathBuf {
    p
}

/// Isi berkas batas memori cgroup. "max" (v2) dan angka raksasa (v1) berarti tanpa batas.
fn cgroup_limit(text: &str) -> Option<u64> {
    text.trim().parse::<u64>().ok().filter(|n| *n < 1 << 60)
}

/// Batas memori container tempat proses ini berjalan, bila ada.
pub fn memory_limit() -> Option<u64> {
    [
        "/sys/fs/cgroup/memory.max",
        "/sys/fs/cgroup/memory/memory.limit_in_bytes",
    ]
    .iter()
    .find_map(|f| fs::read_to_string(f).ok())
    .and_then(|text| cgroup_limit(&text))
}

/// Folder state berada di luar project, supaya tidak ikut tersaji dan tidak mengotori repo.
pub fn state_base() -> PathBuf {
    env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("jamuphp")
}

/// Container dengan filesystem read-only, atau UID yang tidak punya folder sendiri:
/// pakai /tmp, dalam folder yang hanya bisa dibuka pengguna ini.
pub fn fallback_state_base() -> Option<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    let uid = unsafe { libc::geteuid() };
    let tmp = PathBuf::from(format!("/tmp/jamuphp-{uid}"));
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&tmp)
        .ok()?;
    (fs::metadata(&tmp).ok()?.uid() == uid).then_some(tmp)
}

pub fn prepare(cmd: &mut Command) {
    // Grup proses sendiri: Ctrl-C di terminal hanya sampai ke supervisor, yang lalu
    // mematikan anak-anaknya secara berurutan.
    cmd.process_group(0);
    // Bila supervisor mati mendadak, kernel mengirim SIGTERM ke anak.
    unsafe {
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        });
    }
}

pub fn init() {}

/// Mengganti proses ini dengan perintah lain: sinyal, terminal, dan kode keluar langsung
/// menjadi milik perintah itu. Hanya kembali bila gagal, dengan alasannya.
pub fn replace_process(mut cmd: Command) -> String {
    let e = cmd.exec();
    format!(
        "{} tidak bisa dijalankan: {e}",
        cmd.get_program().to_string_lossy()
    )
}

/// Perintah satu baris dari `jamu.toml`, dijalankan shell.
pub fn shell(run: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(run);
    c
}

pub fn ask_to_stop(child: &mut Child, how: Stop) {
    let pid = child.id() as i32;
    let (target, sig) = match how {
        Stop::Term => (pid, libc::SIGTERM),
        Stop::Quit => (pid, libc::SIGQUIT),
        // Setiap anak memimpin grup prosesnya sendiri (lihat `prepare`); pid negatif berarti
        // seluruh grup, yaitu shell dan perintah yang dijalankannya.
        Stop::Group => (-pid, libc::SIGTERM),
    };
    unsafe { libc::kill(target, sig) };
}

/// Mematikan seluruh grup proses, termasuk anak yang tertinggal.
pub fn kill_tree(child: &mut Child) {
    unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
}

/// Sebagai PID 1 di container, proses yatim (misalnya pekerja php-fpm yang induknya mati,
/// atau proses latar yang dijalankan kode situs) menjadi anak kita. Tanpa dipungut, mereka
/// menumpuk sebagai zombie. Anak milik kita sendiri dilewati: itu urusan `try_wait`.
pub fn reap_orphans(ours: &[u32]) {
    if unsafe { libc::getpid() } != 1 {
        return;
    }
    loop {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // WNOWAIT: intip dulu siapa yang selesai, tanpa memungutnya.
        let r = unsafe {
            libc::waitid(
                libc::P_ALL,
                0,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        let pid = unsafe { info.si_pid() };
        if r != 0 || pid == 0 || ours.contains(&(pid as u32)) {
            return;
        }
        unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
    }
}

/// Alasan sebuah folder tidak bisa ditulis PHP, bila ada.
pub fn unwritable(dir: &Path) -> Option<String> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    (unsafe { libc::access(c.as_ptr(), libc::W_OK) } != 0)
        .then(|| format!("uid {}", unsafe { libc::geteuid() }))
}

/// Nama program yang mendengarkan di sebuah port, dicari lewat /proc. Ini usaha terbaik:
/// proses milik pengguna lain tidak terlihat tanpa hak root.
pub fn port_holder(port: u16) -> String {
    let listening_inode = |table: String| {
        table.lines().skip(1).find_map(|line| {
            // Kolom: sl, alamat lokal, alamat lawan, status (0A = LISTEN), ..., inode di indeks 9.
            let f: Vec<&str> = line.split_whitespace().collect();
            let local_port = u16::from_str_radix(f.get(1)?.rsplit(':').next()?, 16).ok()?;
            (local_port == port && f.get(3) == Some(&"0A"))
                .then(|| f.get(9).map(|s| s.to_string()))?
        })
    };
    let inode = ["/proc/net/tcp", "/proc/net/tcp6"]
        .iter()
        .filter_map(|f| fs::read_to_string(f).ok())
        .find_map(listening_inode);
    let Some(inode) = inode else {
        return String::new();
    };
    let target = format!("socket:[{inode}]");
    for proc in fs::read_dir("/proc").into_iter().flatten().flatten() {
        let pid = proc.file_name().to_string_lossy().into_owned();
        if !pid.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        for fd in fs::read_dir(proc.path().join("fd"))
            .into_iter()
            .flatten()
            .flatten()
        {
            if fs::read_link(fd.path()).is_ok_and(|l| l.to_string_lossy() == target) {
                let name = fs::read_to_string(proc.path().join("comm")).unwrap_or_default();
                return format!(" oleh {} (pid {pid})", name.trim());
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batas_memori_cgroup_terbaca() {
        assert_eq!(cgroup_limit("268435456\n"), Some(268435456));
        assert_eq!(cgroup_limit("max\n"), None);
        assert_eq!(cgroup_limit("9223372036854771712\n"), None);
        assert_eq!(cgroup_limit(""), None);
    }
}
