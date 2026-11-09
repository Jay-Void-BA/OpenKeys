//! Bounded background import preparation. This is a host privacy guard, not
//! device authorization. A trusted backend must supply a live Access first.
use eframe::egui;
use std::{
    fs::{File, OpenOptions},
    io::{ErrorKind, Read},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    thread::JoinHandle,
    time::{Instant, SystemTime},
};
use vault_import::{Entry, MAX_ENTRIES, MAX_FILE_BYTES, parse_import};
use zeroize::Zeroizing;

pub const MAX_FILES: usize = 16;

#[derive(Clone, Copy)]
pub struct ReadDeadline {
    pub expires: Instant,
    pub issued_wall: SystemTime,
    pub expires_wall: SystemTime,
}
impl ReadDeadline {
    fn live_at(&self, now: Instant, wall: SystemTime) -> bool {
        now < self.expires && wall >= self.issued_wall && wall < self.expires_wall
    }
    fn check(&self, cancelled: &AtomicBool) -> Result<(), Error> {
        if cancelled.load(Ordering::Acquire) || !self.live_at(Instant::now(), SystemTime::now()) {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Cancelled,
    Busy,
    Worker,
    Files,
    Io,
    Size,
    Format,
    Entries,
}
impl Error {
    pub fn message(self) -> &'static str {
        match self {
            Self::Cancelled => "Import authorization ended. No data was written.",
            Self::Busy => {
                "The previous file operation is still stopping. Try again when it finishes."
            }
            Self::Worker => "The import worker stopped. No data was written.",
            Self::Files => "Choose between 1 and 16 regular JSON files.",
            Self::Io => "Cannot safely read a selected file. No data was written.",
            Self::Size => "The whole import batch must be at most 1 MiB of JSON.",
            Self::Format => "Import rejected. Check the format and limits. No data was written.",
            Self::Entries => "The whole import batch must contain at most 256 entries.",
        }
    }
}

struct Task {
    generation: u64,
    cancelled: Arc<AtomicBool>,
    result: Option<Receiver<Result<Vec<Entry>, Error>>>,
    thread: JoinHandle<()>,
}

/// At most one thread, including a cancelled thread stuck in an OS file call.
/// Never joins an unfinished thread on the UI thread. Cancel drops the result
/// receiver immediately, so queued or late plaintext cannot enter a new lease.
#[derive(Default)]
pub struct ImportWorker {
    task: Option<Task>,
}
impl ImportWorker {
    pub fn busy(&self) -> bool {
        self.task.is_some()
    }
    pub fn cancel(&mut self) {
        if let Some(task) = &mut self.task {
            task.cancelled.store(true, Ordering::Release);
            task.result = None;
        }
    }
    fn reap(&mut self) {
        if self
            .task
            .as_ref()
            .is_some_and(|task| task.result.is_none() && task.thread.is_finished())
            && let Some(task) = self.task.take()
        {
            // Already finished: joining here cannot wait for a file operation.
            let _ = task.thread.join();
        }
    }
    pub fn start(
        &mut self,
        paths: Vec<PathBuf>,
        deadline: ReadDeadline,
        generation: u64,
        ctx: egui::Context,
    ) -> Result<(), Error> {
        self.reap();
        if self.busy() {
            return Err(Error::Busy);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        deadline.check(&cancelled)?;
        let worker_cancelled = cancelled.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("openkeys-import".into())
            .spawn(move || {
                let result = prepare(paths, deadline, &worker_cancelled);
                // A cancelled receiver never receives entries. Entry strings
                // and input buffers zeroize when dropped; no path/error detail
                // or secret is logged or copied into a user-facing message.
                let result = if deadline.check(&worker_cancelled).is_ok() {
                    result
                } else {
                    Err(Error::Cancelled)
                };
                let _ = tx.send(result);
                ctx.request_repaint();
            })
            .map_err(|_| Error::Worker)?;
        self.task = Some(Task {
            generation,
            cancelled,
            result: Some(rx),
            thread,
        });
        Ok(())
    }
    pub fn poll(&mut self) -> Option<(u64, Result<Vec<Entry>, Error>)> {
        let result = self.task.as_mut().and_then(|task| {
            let received = task.result.as_ref()?.try_recv();
            let result = match received {
                Ok(result) => result,
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => Err(Error::Worker),
            };
            task.result = None;
            Some((task.generation, result))
        });
        self.reap();
        result
    }
}
impl Drop for ImportWorker {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn regular_file(path: &std::path::Path) -> Result<File, Error> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // A FIFO/device cannot block the open before the type check. Refuse
        // final symlinks rather than following them to a different file type.
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Open the reparse point itself, then reject it; forbid concurrent
        // sharing for this handle. Constants are Win32 file API flags.
        options.custom_flags(0x0020_0000).share_mode(0);
    }
    let file = options.open(path).map_err(|_| Error::Io)?;
    let meta = file.metadata().map_err(|_| Error::Io)?;
    if !meta.file_type().is_file() {
        return Err(Error::Files);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x0000_0400 != 0 {
            return Err(Error::Files);
        }
    }
    Ok(file)
}

fn read_bounded(
    reader: &mut impl Read,
    budget: usize,
    deadline: ReadDeadline,
    cancelled: &AtomicBool,
) -> Result<Zeroizing<Vec<u8>>, Error> {
    // Avoid reallocating an input buffer containing plaintext. At most 1 MiB
    // is reserved, and only one worker may exist in the application.
    let mut data = Zeroizing::new(Vec::with_capacity(budget));
    let mut chunk = Zeroizing::new([0u8; 8192]);
    loop {
        deadline.check(cancelled)?;
        let bound = chunk.len().min(budget - data.len() + 1);
        let count = match reader.read(&mut chunk[..bound]) {
            Ok(count) => count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return Err(Error::Io),
        };
        deadline.check(cancelled)?;
        if count == 0 {
            return Ok(data);
        }
        if count > budget - data.len() {
            return Err(Error::Size);
        }
        data.extend_from_slice(&chunk[..count]);
    }
}

fn prepare(
    paths: Vec<PathBuf>,
    deadline: ReadDeadline,
    cancelled: &AtomicBool,
) -> Result<Vec<Entry>, Error> {
    deadline.check(cancelled)?;
    if paths.is_empty() || paths.len() > MAX_FILES {
        return Err(Error::Files);
    }
    let mut entries = Vec::new();
    let mut budget = MAX_FILE_BYTES;
    for path in paths {
        deadline.check(cancelled)?;
        let mut file = regular_file(&path)?;
        deadline.check(cancelled)?;
        if file.metadata().map_err(|_| Error::Io)?.len() > budget as u64 {
            return Err(Error::Size);
        }
        let data = read_bounded(&mut file, budget, deadline, cancelled)?;
        budget -= data.len();
        let parsed = parse_import(&data).map_err(|_| Error::Format)?;
        deadline.check(cancelled)?;
        if parsed.len() > MAX_ENTRIES - entries.len() {
            return Err(Error::Entries);
        }
        entries.extend(parsed);
    }
    deadline.check(cancelled)?;
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn deadline() -> ReadDeadline {
        let wall = SystemTime::now();
        ReadDeadline {
            expires: Instant::now() + Duration::from_secs(30),
            issued_wall: wall,
            expires_wall: wall + Duration::from_secs(30),
        }
    }
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "openkeys-import-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    const NOTE: &[u8] =
        br#"{"version":2,"entries":[{"type":"note","title":"fixture","body":"public-test-value"}]}"#;
    #[test]
    fn bounded_reader_rechecks_cancellation_after_the_os_read() {
        struct Cancelling<'a>(&'a AtomicBool);
        impl Read for Cancelling<'_> {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                buf[..4].copy_from_slice(b"test");
                self.0.store(true, Ordering::Release);
                Ok(4)
            }
        }
        let cancelled = AtomicBool::new(false);
        assert!(matches!(
            read_bounded(&mut Cancelling(&cancelled), 32, deadline(), &cancelled),
            Err(Error::Cancelled)
        ));
        assert_eq!(
            read_bounded(&mut &b"12345"[..], 4, deadline(), &AtomicBool::new(false)),
            Err(Error::Size)
        );
        assert_eq!(
            read_bounded(&mut &b"1234"[..], 4, deadline(), &AtomicBool::new(false))
                .unwrap()
                .as_slice(),
            b"1234"
        );
    }
    #[test]
    fn deadline_denies_sleep_elapsed_wall_time_and_rollback() {
        let d = deadline();
        let now = Instant::now();
        assert!(d.live_at(now, d.issued_wall));
        assert!(!d.live_at(d.expires, d.issued_wall));
        assert!(!d.live_at(now, d.expires_wall));
        assert!(!d.live_at(now, d.issued_wall - Duration::from_secs(1)));
    }
    #[test]
    fn batch_limits_are_aggregate_and_errors_never_return_a_partial_batch() {
        let f = Fixture::new();
        let good = f.file("a.json", NOTE);
        let bad = f.file("b.json", b"invalid");
        let cancelled = AtomicBool::new(false);
        assert_eq!(
            prepare(vec![good.clone()], deadline(), &cancelled)
                .unwrap()
                .len(),
            1
        );
        assert!(matches!(
            prepare(vec![good.clone(), bad], deadline(), &cancelled),
            Err(Error::Format)
        ));
        assert!(matches!(
            prepare(vec![good.clone(); MAX_FILES + 1], deadline(), &cancelled),
            Err(Error::Files)
        ));
        let mut padded = NOTE.to_vec();
        padded.resize(MAX_FILE_BYTES / 2 + 1, b' ');
        let large = f.file("large.json", &padded);
        assert!(matches!(
            prepare(vec![large.clone(), large], deadline(), &cancelled),
            Err(Error::Size)
        ));
        let item = r#"{"type":"note","title":"x","body":"x"}"#;
        let json = format!(
            "{{\"version\":2,\"entries\":[{}]}}",
            vec![item; 129].join(",")
        );
        let many = f.file("many.json", json.as_bytes());
        assert!(matches!(
            prepare(vec![many.clone(), many], deadline(), &cancelled),
            Err(Error::Entries)
        ));
        assert!(matches!(
            prepare(vec![f.0.clone()], deadline(), &cancelled),
            Err(Error::Files | Error::Io)
        ));
    }
    #[test]
    fn final_symlinks_and_special_files_are_refused() {
        #[cfg(unix)]
        {
            let f = Fixture::new();
            let path = f.file("real.json", NOTE);
            let link = f.0.join("link.json");
            std::os::unix::fs::symlink(path, &link).unwrap();
            assert!(regular_file(&link).is_err());
            assert!(matches!(
                regular_file(std::path::Path::new("/dev/null")),
                Err(Error::Files)
            ));
        }
        #[cfg(windows)]
        assert!(regular_file(std::path::Path::new("NUL")).is_err());
    }
    #[test]
    fn cancel_drops_queued_and_late_results_and_preserves_one_thread_limit() {
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let (tx, rx) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let thread = std::thread::spawn(move || {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            let _ = tx.send(parse_import(NOTE).map_err(|_| Error::Format));
        });
        entered_rx.recv().unwrap();
        let mut worker = ImportWorker {
            task: Some(Task {
                generation: 17,
                cancelled: cancelled.clone(),
                result: Some(rx),
                thread,
            }),
        };
        worker.cancel();
        assert!(cancelled.load(Ordering::Acquire));
        assert!(worker.poll().is_none());
        assert!(worker.busy());
        assert_eq!(
            worker.start(Vec::new(), deadline(), 18, egui::Context::default()),
            Err(Error::Busy)
        );
        release_tx.send(()).unwrap();
        // Joining is permitted in this test, never for an unfinished UI job.
        let task = worker.task.take().unwrap();
        task.thread.join().unwrap();
        assert!(task.result.is_none());
        assert!(worker.poll().is_none());
        assert!(!worker.busy());
    }
    #[test]
    fn a_real_background_read_delivers_only_its_original_generation() {
        let f = Fixture::new();
        let path = f.file("fixture.json", NOTE);
        let mut worker = ImportWorker::default();
        worker
            .start(vec![path], deadline(), 28, egui::Context::default())
            .unwrap();
        let limit = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some((generation, entries)) = worker.poll() {
                assert_eq!(generation, 28);
                assert_eq!(entries.unwrap().len(), 1);
                break;
            }
            assert!(Instant::now() < limit, "background fixture read timed out");
            std::thread::yield_now();
        }
        worker.cancel();
    }
}
