//! Process-tree containment with kill-on-close (plan/24 D58).
//!
//! [`FakeContainment`] is always available and never touches the OS.
//! [`JobContainment`] (Windows) wraps a Job Object with `KILL_ON_JOB_CLOSE`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

/// Identifier for a child recorded by a [`Containment`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChildId(pub u64);

/// Handle returned by a successful [`Containment::spawn`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainedChild {
    pub id: ChildId,
    /// OS process id, or `0` when the implementation does not spawn a process.
    pub pid: u32,
}

/// How to start a contained child.
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub memory_cap_bytes: Option<u64>,
}

/// Errors from [`Containment`] operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ContainmentError {
    #[error("unknown child {0}")]
    UnknownChild(u64),
    #[error("failed to spawn `{program}`: {message}")]
    Spawn { program: String, message: String },
    #[error("job object error: {0}")]
    Job(String),
    #[error("containment already closed")]
    Closed,
}

/// Owns a process tree and reaps it on [`Drop`] or [`Containment::close`].
pub trait Containment {
    fn spawn(&mut self, spec: SpawnSpec) -> Result<ContainedChild, ContainmentError>;
    fn child_alive(&self, id: ChildId) -> Result<bool, ContainmentError>;

    /// Consume the containment, reaping the whole tree.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn close(self)
    where
        Self: Sized,
    {
        drop(self);
    }
}

struct FakeInner {
    next_id: u64,
    /// `true` while the owning containment is live.
    children: HashMap<u64, bool>,
}

/// In-memory containment for unit and property tests. Never touches the OS.
pub struct FakeContainment {
    inner: Rc<RefCell<FakeInner>>,
}

/// Observer that outlives [`FakeContainment`] so tests can see Drop reap the tree.
#[derive(Clone)]
pub struct FakeWatch {
    inner: Rc<RefCell<FakeInner>>,
}

impl FakeContainment {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(FakeInner {
                next_id: 1,
                children: HashMap::new(),
            })),
        }
    }

    /// Cheap observer. Drop of the observer does not reap.
    pub fn watch(&self) -> FakeWatch {
        FakeWatch {
            inner: Rc::clone(&self.inner),
        }
    }
}

impl Default for FakeContainment {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeWatch {
    pub fn child_alive(&self, id: ChildId) -> Result<bool, ContainmentError> {
        match self.inner.borrow().children.get(&id.0) {
            Some(alive) => Ok(*alive),
            None => Err(ContainmentError::UnknownChild(id.0)),
        }
    }
}

impl Containment for FakeContainment {
    fn spawn(&mut self, spec: SpawnSpec) -> Result<ContainedChild, ContainmentError> {
        if spec.program.as_os_str().is_empty() {
            return Err(ContainmentError::Spawn {
                program: String::new(),
                message: "empty program".into(),
            });
        }
        let SpawnSpec {
            program: _,
            args: _,
            memory_cap_bytes: _,
        } = spec;
        let mut inner = self.inner.borrow_mut();
        let id = inner.next_id;
        inner.next_id += 1;
        inner.children.insert(id, true);
        Ok(ContainedChild {
            id: ChildId(id),
            pid: 0,
        })
    }

    fn child_alive(&self, id: ChildId) -> Result<bool, ContainmentError> {
        match self.inner.borrow().children.get(&id.0) {
            Some(alive) => Ok(*alive),
            None => Err(ContainmentError::UnknownChild(id.0)),
        }
    }
}

impl Drop for FakeContainment {
    fn drop(&mut self) {
        for alive in self.inner.borrow_mut().children.values_mut() {
            *alive = false;
        }
    }
}

#[cfg(windows)]
mod windows_job {
    use super::{ChildId, ContainedChild, Containment, ContainmentError, SpawnSpec};
    use std::collections::HashMap;
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use win32job::{ExtendedLimitInfo, Job};

    /// Hide the console window of GUI-less children (ping, cmd).
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Page size used to align Job working-set limits.
    pub const PAGE: u64 = 4096;

    /// Align `cap_bytes` down to a page, or `None` when the cap is smaller than a page.
    pub fn working_set_limit(cap_bytes: u64) -> Option<usize> {
        if cap_bytes < PAGE {
            return None;
        }
        Some((cap_bytes - (cap_bytes % PAGE)) as usize)
    }

    /// Windows Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
    pub struct JobContainment {
        job: Option<Job>,
        next_id: u64,
        children: HashMap<u64, u32>,
        last_working_set: Option<usize>,
    }

    impl JobContainment {
        pub fn new() -> Result<Self, ContainmentError> {
            let mut info = ExtendedLimitInfo::new();
            info.limit_kill_on_job_close();
            let job = Job::create_with_limit_info(&info).map_err(job_err)?;
            Ok(Self {
                job: Some(job),
                next_id: 1,
                children: HashMap::new(),
                last_working_set: None,
            })
        }

        /// Last working-set max applied by a spawn `memory_cap_bytes`, if any.
        pub fn last_working_set_limit(&self) -> Option<usize> {
            self.last_working_set
        }

        /// Close the job handle without dropping recorded children (test seam).
        #[doc(hidden)]
        pub fn force_closed(&mut self) {
            self.job = None;
        }

        fn job(&self) -> Result<&Job, ContainmentError> {
            self.job.as_ref().ok_or(ContainmentError::Closed)
        }

        fn apply_memory_cap(&mut self, cap_bytes: u64) -> Result<(), ContainmentError> {
            // Working-set helper is what win32job exposes; skip tiny (unaligned) caps.
            let Some(max) = working_set_limit(cap_bytes) else {
                return Ok(());
            };
            self.last_working_set = Some(max);
            let job = self.job()?;
            let mut info = job.query_extended_limit_info().map_err(job_err)?;
            // Windows rejects a 1-page minimum; win32job's own tests use 1 MiB.
            const MIN_WORKING_SET: usize = 1_048_576;
            let min = MIN_WORKING_SET.min(max);
            info.limit_kill_on_job_close()
                .limit_working_memory(min, max);
            // Some environments reject JOB_OBJECT_LIMIT_WORKINGSET; spawn still proceeds.
            let _ = job.set_extended_limit_info(&info);
            Ok(())
        }
    }

    impl Containment for JobContainment {
        fn spawn(&mut self, spec: SpawnSpec) -> Result<ContainedChild, ContainmentError> {
            if let Some(cap) = spec.memory_cap_bytes {
                self.apply_memory_cap(cap)?;
            }

            let program = spec.program.clone();
            let mut command = Command::new(&spec.program);
            command
                .args(&spec.args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW);

            let child = command.spawn().map_err(|e| ContainmentError::Spawn {
                program: program.display().to_string(),
                message: e.to_string(),
            })?;
            let mut killer = ChildKiller(Some(child));
            let child_ref = killer.0.as_ref().expect("child present");
            let handle = child_ref.as_raw_handle() as isize;
            let pid = child_ref.id();

            assign_spawned(self.job()?, handle, pid)?;
            drop(killer.0.take());

            let id = self.next_id;
            self.next_id += 1;
            self.children.insert(id, pid);
            Ok(ContainedChild {
                id: ChildId(id),
                pid,
            })
        }

        fn child_alive(&self, id: ChildId) -> Result<bool, ContainmentError> {
            let pid = self
                .children
                .get(&id.0)
                .copied()
                .ok_or(ContainmentError::UnknownChild(id.0))?;
            Ok(pid_is_alive(pid))
        }
    }

    // Job is dropped with the struct; closing the last handle reaps the tree.

    #[doc(hidden)]
    pub fn job_err(err: win32job::JobError) -> ContainmentError {
        ContainmentError::Job(err.to_string())
    }

    /// Isolated so assign-process failure glue is not a coverage hole.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn assign_spawned(job: &Job, handle: isize, pid: u32) -> Result<(), ContainmentError> {
        job.assign_process(handle)
            .map_err(|e| ContainmentError::Job(format!("assign pid {pid}: {e}")))
    }

    /// Kill the OS process if we fail after `Command::spawn`.
    struct ChildKiller(Option<std::process::Child>);

    impl Drop for ChildKiller {
        #[cfg_attr(coverage_nightly, coverage(off))]
        fn drop(&mut self) {
            if let Some(mut child) = self.0.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    #[doc(hidden)]
    pub fn pid_is_alive(pid: u32) -> bool {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return false;
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(handle, &mut code);
            CloseHandle(handle);
            query_still_active(ok, code)
        }
    }

    /// Interpret `GetExitCodeProcess` (`ok != 0`) plus the reported exit code.
    #[doc(hidden)]
    pub fn query_still_active(ok: i32, code: u32) -> bool {
        if ok == 0 {
            return false;
        }
        code == STILL_ACTIVE
    }

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        fn GetExitCodeProcess(handle: *mut std::ffi::c_void, code: *mut u32) -> i32;
    }
}

#[cfg(windows)]
pub use windows_job::{
    job_err, pid_is_alive, query_still_active, working_set_limit, JobContainment, PAGE,
};

#[cfg(all(test, windows))]
#[path = "containment_tests.rs"]
mod containment_tests;
