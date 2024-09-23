use std::{ffi::{c_void, CString}, io::Write, path::{Path, PathBuf}, process::{Child, Command}, thread, time::Duration};
use crate::Error;

extern "C" {
    fn init_perfetto(backend: u32, output_path: *const i8, buffer_size: usize) -> *mut c_void;
    fn deinit_perfetto(guard: *mut c_void);
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    /// creates a tracing service on a dedicated thread.
    InProcess = 1,
    /// requires the traced service to be running separately. is used to collect a fused trace
    System = 2,
}

/// Backend configuration for perfetto.
pub enum BackendConfig {
    /// Use API to create a trace of the local process.
    InProcess { 
        /// Size of the buffer in kilobytes.
        buffer_size_kb: usize 
    },
    /// Use system wide tracing fused with the local process data.
    /// The `PerfettoGuard` will take care of starting and stopping the perfetto processes.
    System {
        /// Path to the perfetto binaries: `perfetto`, `traced`, `traced_probes`.
        /// If `None`, the system path will be used.
        perfetto_bin_path: Option<String>,
        /// Path to the perfetto config file.
        /// If none the default one `config/system_profiling.cfg` will be used.
        perfetto_cfg_path: Option<String>
    },
}

impl BackendConfig {
    fn backend(&self) -> Backend {
        match self {
            BackendConfig::InProcess { .. } => Backend::InProcess,
            BackendConfig::System { .. } => Backend::System,
        }
    }

    fn buffer_size_kb(&self) -> usize {
        match self {
            BackendConfig::InProcess { buffer_size_kb } => *buffer_size_kb,
            BackendConfig::System { .. } => 0,
        }
    }
}

/// Create only one of these per tracing session. It should live for the duration of the program.
pub struct PerfettoGuard {
    ptr: *mut c_void,
    processes: Option<PerfettoProcessesGuard>,
}

// Safety: the pointers here are heap allocated and not shared. Should be ok to send them to other threads
unsafe impl Send for PerfettoGuard {}
unsafe impl Sync for PerfettoGuard {}

impl PerfettoGuard {
    /// Initializes tracing. 
    pub fn new(backend: BackendConfig, output_path: &str) -> Result<Self, Error> {
        let processes = match &backend {
            BackendConfig::System { perfetto_bin_path, perfetto_cfg_path } => {
                Some(PerfettoProcessesGuard::new(perfetto_bin_path.as_ref().map(|s| s.as_str()), output_path, perfetto_cfg_path.as_ref().map(|s| s.as_str()))?)
            },
            BackendConfig::InProcess { .. } => {
                None
            },
        };
        
        let output_path = CString::new(output_path).expect("output_path is not a valid string");
        let buffer_size_kb = backend.buffer_size_kb();
        let backend = backend.backend();
        let ptr = unsafe { init_perfetto(backend as u32, output_path.as_ptr(), buffer_size_kb) };
        
        
        Ok(Self { ptr, processes })
    }
}

impl Drop for PerfettoGuard {
    fn drop(&mut self) {
        // in wrapper.cc there's a 2 second flush interval. want to ensure all logs are flushed before stopping perfetto.
        std::thread::sleep(Duration::from_millis(2500));
        unsafe { deinit_perfetto(self.ptr) }

        self.processes.take().map(|mut processes| {
            _ = processes.stop_and_wait().expect("failed to stop perfetto processes");
        });
    }
}

struct PerfettoProcessesGuard {
    perfetto: ProcessGuard,
    traced_probes: ProcessGuard,
    traced: ProcessGuard,
    _temp_cfg: Option<tempfile::NamedTempFile>,
}

impl PerfettoProcessesGuard {
    fn new(bin_folder: Option<&str>, output_path: &str, config: Option<&str>) -> Result<Self, Error> {
        let traced_probes = ProcessGuard::new("traced_probes".to_string(), Command::new(join_with_folder(bin_folder.clone(), "traced_probes")))?;
        let traced = ProcessGuard::new("traced".to_string(), Command::new(join_with_folder(bin_folder.clone(), "traced")))?;
        
        let mut perfetto = Command::new(join_with_folder(bin_folder.clone(), "perfetto"));
        perfetto
            .arg("--txt")
            .arg("-o")
            .arg(output_path)
            .arg("-c");
        let _temp_cfg = match config {
            Some(config) => {
                perfetto.arg(config);
                None
            }
            None => {
                let mut tmp_cfg = tempfile::NamedTempFile::new()?;
                tmp_cfg.write_all(include_str!("../config/system_profiling.cfg").as_bytes())?;
                tmp_cfg.flush()?;
                perfetto.arg(tmp_cfg.path().to_str().expect("invalid path"));

                Some(tmp_cfg)
            }
        };
        let perfetto = ProcessGuard::new("perfetto".to_string(), perfetto)?;

        Ok(Self { perfetto, traced_probes, traced, _temp_cfg })
    }

    fn stop_and_wait(&mut self) -> Result<(), Error> {
        thread::sleep(Duration::from_millis(1000));
        self.perfetto.stop_and_wait()?;
        thread::sleep(Duration::from_millis(1000));
        self.traced_probes.stop_and_wait()?;
        thread::sleep(Duration::from_millis(1000));
        self.traced.stop_and_wait()?;
        Ok(())
    }
}

fn join_with_folder(folder: Option<&str>, binary: &str) -> PathBuf {
    match folder {
        Some(folder) => Path::new(folder).join(binary),
        None => Path::new(binary).into(),
    }
}

struct ProcessGuard {
    process: Option<Child>,
    name: String,
}

impl ProcessGuard {
    fn new(name: String, mut command: Command) -> Result<Self, Error> {
        command.spawn().map(|process| Self { process: Some(process), name: name.clone() }).map_err(|e| Error::ProcessError(name, e))
    }

    fn stop_and_wait(&mut self) -> Result<(), Error> {
        let Some(process) = self.process.take() else {
            return Ok(());
        };

        // try to finish the process gracefully
        let res = unsafe {libc::kill(process.id() as i32, libc::SIGINT)};
        if res != 0 {
            return Err(Error::ProcessError(self.name.clone(), std::io::Error::last_os_error()));
        }

        Ok(())
    }
}

impl Drop for PerfettoProcessesGuard {
    fn drop(&mut self) {
        _ = self.perfetto.stop_and_wait();
    }
}