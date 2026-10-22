//! One smep per user: a second launch hands its files to the window that is
//! already open, which shows each in a tab, and exits.
//!
//! The running smep listens on a local socket named after its config
//! directory (a named pipe on Windows, a socket file in the config directory
//! elsewhere), so a separate `SMEP_CONFIG_DIR` is a separate smep. The
//! exchange is lines of text:
//!
//! 1. The listener sends its process id (on Windows the caller, which the
//!    user just started and so may bring a window forward, lets that process
//!    do it).
//! 2. The caller sends one absolute path per line, then an empty line.
//!    No paths asks for an empty tab.
//! 3. The listener answers `ok` once the paths are queued for the window.
//!
//! Anything else (no listener, a path that is not UTF-8, a broken exchange)
//! and the caller opens a window of its own, as smep always did.

use std::io::{self, BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use interprocess::local_socket::{ListenerOptions, Name, Stream, prelude::*};

/// The socket for the smep whose config directory is `dir`.
pub fn name(dir: &Path) -> io::Result<Name<'static>> {
    #[cfg(windows)]
    {
        use interprocess::local_socket::GenericNamespaced;
        // Pipe names are machine-wide: the user and the directory, hashed,
        // keep users and config directories apart.
        let user = std::env::var("USERNAME").unwrap_or_default();
        let key = format!("{user}\n{}", dir.display()).to_lowercase();
        format!("smep-{:016x}", fnv1a(key.as_bytes())).to_ns_name::<GenericNamespaced>()
    }
    #[cfg(not(windows))]
    {
        use interprocess::local_socket::GenericFilePath;
        // A socket file in the user's own directory: nobody else can reach
        // it. Socket paths are limited to about 100 bytes; a longer
        // directory falls back to one under the temporary directory.
        let mut path = dir.join("smep.sock");
        if path.as_os_str().len() > 100 {
            path = std::env::temp_dir().join(format!(
                "smep-{:016x}.sock",
                fnv1a(dir.as_os_str().as_encoded_bytes())
            ));
        }
        std::fs::create_dir_all(path.parent().unwrap_or(Path::new("/")))?;
        path.to_fs_name::<GenericFilePath>()
            .map(|name| name.into_owned())
    }
}

/// What became of this launch.
#[derive(Debug, PartialEq, Eq)]
pub enum Claim {
    /// A running smep took the files: this launch is done.
    HandedOver,
    /// This launch is the smep: later launches come to `open`.
    Listening,
    /// Neither (a smep that refused, a socket that cannot be made): this
    /// launch opens its own window and does not listen.
    Alone,
}

/// Hand `paths` to the smep listening on `name`, or become that smep,
/// handing every later launch's paths to `open`.
///
/// Launches can race (Explorer starts one per file when several are opened
/// at once): the first to make the socket listens and the others, finding
/// it taken, hand over to it once it answers. A socket file a dead smep
/// left behind is taken over after that second: nothing answers on it.
pub fn claim(
    name: Name<'static>,
    paths: &[PathBuf],
    open: impl Fn(Vec<PathBuf>) + Clone + Send + Sync + 'static,
) -> Claim {
    match hand_over(&name, paths) {
        Handed::Taken => return Claim::HandedOver,
        Handed::Refused => return Claim::Alone,
        Handed::NoListener => {}
    }
    if listen(name.clone(), false, open.clone()).is_ok() {
        return Claim::Listening;
    }
    for _ in 0..20 {
        match hand_over(&name, paths) {
            Handed::Taken => return Claim::HandedOver,
            Handed::Refused => return Claim::Alone,
            Handed::NoListener => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    match listen(name, true, open) {
        Ok(()) => Claim::Listening,
        Err(_) => Claim::Alone,
    }
}

/// How a hand-over went.
#[derive(Debug, PartialEq, Eq)]
enum Handed {
    Taken,
    /// Nothing answered on the socket.
    NoListener,
    /// Something answered and the exchange failed (or the paths cannot
    /// be sent): a smep is there, but this launch has to fend for itself.
    Refused,
}

/// How long a hand-over may take before this launch gives up on the
/// running smep and opens a window of its own.
const HAND_OVER_TIMEOUT: Duration = Duration::from_secs(3);

fn hand_over(name: &Name<'_>, paths: &[PathBuf]) -> Handed {
    let Ok(stream) = Stream::connect(name.borrow()) else {
        return Handed::NoListener;
    };
    // The exchange runs in a thread of its own, so a smep that stops
    // answering costs this launch the timeout, not its life.
    let paths = paths.to_vec();
    let (done, outcome) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("smep-hand-over".into())
        .spawn(move || {
            let _ = done.send(exchange(stream, &paths));
        });
    match spawned.map(|_| outcome.recv_timeout(HAND_OVER_TIMEOUT)) {
        Ok(Ok(Ok(true))) => Handed::Taken,
        _ => Handed::Refused,
    }
}

fn exchange(stream: Stream, paths: &[PathBuf]) -> io::Result<bool> {
    let mut lines = Vec::with_capacity(paths.len());
    for path in paths {
        let Some(line) = path.to_str().filter(|line| !line.contains('\n')) else {
            return Ok(false);
        };
        lines.push(line);
    }
    let mut stream = BufReader::new(stream);
    let mut pid = String::new();
    stream.read_line(&mut pid)?;
    if let Ok(pid) = pid.trim().parse::<u32>() {
        allow_to_come_forward(pid);
    }
    let mut request = String::new();
    for line in lines {
        request.push_str(line);
        request.push('\n');
    }
    request.push('\n');
    stream.get_mut().write_all(request.as_bytes())?;
    let mut answer = String::new();
    stream.read_line(&mut answer)?;
    Ok(answer.trim() == "ok")
}

/// Listen on `name` in a thread of its own, handing every request's paths
/// to `open`. Fails when the socket cannot be made, which `replace` turns
/// into taking over a socket file left behind (Unix).
fn listen(
    name: Name<'static>,
    replace: bool,
    open: impl Fn(Vec<PathBuf>) + Send + Sync + 'static,
) -> io::Result<()> {
    let listener = ListenerOptions::new()
        .name(name)
        .try_overwrite(replace)
        .create_sync()?;
    std::thread::Builder::new()
        .name("smep-instance".into())
        .spawn(move || {
            let open = std::sync::Arc::new(open);
            for stream in listener.incoming().filter_map(Result::ok) {
                // A thread per caller, so one that connects and never
                // finishes holds up nobody else. A broken exchange is the
                // caller's problem: it opens a window of its own.
                let open = open.clone();
                let _ = std::thread::Builder::new()
                    .name("smep-instance-caller".into())
                    .spawn(move || serve(stream, &*open));
            }
        })?;
    Ok(())
}
fn serve(stream: Stream, open: &(impl Fn(Vec<PathBuf>) + ?Sized)) -> io::Result<()> {
    let mut stream = BufReader::new(stream);
    writeln!(stream.get_mut(), "{}", std::process::id())?;
    let mut paths = Vec::new();
    loop {
        let mut line = String::new();
        if stream.read_line(&mut line)? == 0 {
            return Ok(()); // the caller went away mid-request
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        paths.push(PathBuf::from(line));
    }
    open(paths);
    stream.get_mut().write_all(b"ok\n")
}

/// Let process `pid` bring its window to the front, which Windows allows
/// only the program the user is working with (this one, just started).
#[cfg(windows)]
fn allow_to_come_forward(pid: u32) {
    // SAFETY: a plain Win32 call with a process id; failure changes nothing.
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(pid);
    }
}

#[cfg(not(windows))]
fn allow_to_come_forward(_pid: u32) {}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smep-instance-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn channel() -> (
        impl Fn(Vec<PathBuf>) + Clone + Send + Sync + 'static,
        mpsc::Receiver<Vec<PathBuf>>,
    ) {
        let (sent, received) = mpsc::channel();
        let sent = std::sync::Mutex::new(sent);
        let sent = std::sync::Arc::new(sent);
        (
            move |paths| sent.lock().unwrap().send(paths).unwrap(),
            received,
        )
    }

    #[test]
    fn a_second_launch_hands_its_paths_to_the_first() {
        let dir = temp_dir("hand");
        let name = name(&dir).unwrap();
        assert_eq!(hand_over(&name, &[]), Handed::NoListener);

        let (open, received) = channel();
        assert_eq!(claim(name.clone(), &[], open.clone()), Claim::Listening);
        let paths = vec![dir.join("a.md"), dir.join("b c.md")];
        assert_eq!(claim(name.clone(), &paths, open.clone()), Claim::HandedOver);
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap(),
            paths
        );
        // No paths: a request for an empty tab.
        assert_eq!(claim(name.clone(), &[], open.clone()), Claim::HandedOver);
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap(),
            Vec::<PathBuf>::new()
        );
        // A path with a line break cannot be sent as a line: this launch
        // opens its own window, without taking the socket from the smep
        // that has it.
        assert_eq!(
            claim(name.clone(), &[PathBuf::from("a\nb.md")], open.clone()),
            Claim::Alone
        );
        assert!(received.recv_timeout(Duration::from_millis(200)).is_err());
        assert_eq!(claim(name, &[dir.join("c.md")], open), Claim::HandedOver);
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap(),
            [dir.join("c.md")]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn launches_at_the_same_moment_end_in_one_smep() {
        let dir = temp_dir("race");
        let name = name(&dir).unwrap();
        let (open, received) = channel();
        let launches: Vec<_> = (0..4)
            .map(|ix| {
                let (name, open) = (name.clone(), open.clone());
                let path = dir.join(format!("{ix}.md"));
                std::thread::spawn(move || claim(name, &[path], open))
            })
            .collect();
        let claims: Vec<Claim> = launches.into_iter().map(|l| l.join().unwrap()).collect();
        let listening = claims.iter().filter(|c| **c == Claim::Listening).count();
        assert_eq!(listening, 1, "{claims:?}");
        assert_eq!(
            claims.iter().filter(|c| **c == Claim::HandedOver).count(),
            3,
            "{claims:?}"
        );
        for _ in 0..3 {
            received.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_dead_smeps_socket_file_is_taken_over() {
        let dir = temp_dir("dead");
        // A socket bound and dropped leaves its file, which nothing answers.
        drop(std::os::unix::net::UnixListener::bind(dir.join("smep.sock")).unwrap());
        assert!(dir.join("smep.sock").exists());
        let (open, _received) = channel();
        let name = name(&dir).unwrap();
        assert_eq!(claim(name.clone(), &[], open.clone()), Claim::Listening);
        assert_eq!(claim(name, &[], open), Claim::HandedOver);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_caller_that_never_finishes_holds_up_nobody() {
        let dir = temp_dir("stall");
        let name = name(&dir).unwrap();
        let (open, received) = channel();
        assert_eq!(claim(name.clone(), &[], open.clone()), Claim::Listening);
        // Connected, and silent from then on.
        let _stalled = Stream::connect(name.borrow()).unwrap();
        let path = dir.join("after.md");
        assert_eq!(
            claim(name, std::slice::from_ref(&path), open),
            Claim::HandedOver
        );
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap(),
            [path]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_directories_are_separate_smeps() {
        let one = name(&temp_dir("one")).unwrap();
        let two = name(&temp_dir("two")).unwrap();
        assert_ne!(format!("{one:?}"), format!("{two:?}"));
        let (open, _received) = channel();
        assert_eq!(claim(one.clone(), &[], open), Claim::Listening);
        assert_eq!(hand_over(&one, &[]), Handed::Taken);
        assert_eq!(
            hand_over(&two, &[]),
            Handed::NoListener,
            "the other directory has no smep"
        );
    }
}
