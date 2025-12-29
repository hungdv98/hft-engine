use std::io;

pub fn pin_to_cpu(core_id: usize) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        pin_to_cpu_linux(core_id)
    }

    #[cfg(target_os = "windows")]
    {
        pin_to_cpu_windows(core_id)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = core_id;
        eprintln!("Warning: CPU pinning not supported on this platform");
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn pin_to_cpu_linux(core_id: usize) -> io::Result<()> {
    use libc::{CPU_SET, CPU_ZERO, cpu_set_t, pthread_self, pthread_setaffinity_np};
    use std::mem;

    unsafe {
        let mut cpuset: cpu_set_t = mem::zeroed();
        CPU_ZERO(&mut cpuset);
        CPU_SET(core_id, &mut cpuset);

        let result = pthread_setaffinity_np(pthread_self(), mem::size_of::<cpu_set_t>(), &cpuset);

        if result != 0 {
            return Err(io::Error::from_raw_os_error(result));
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn pin_to_cpu_windows(core_id: usize) -> io::Result<()> {
    use windows::Win32::System::Threading::{GetCurrentThread, SetThreadAffinityMask};

    unsafe {
        let mask: usize = 1 << core_id;
        let result = SetThreadAffinityMask(GetCurrentThread(), mask);

        if result == 0 {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
}

pub fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_num_cpus() {
        let cpus = num_cpus();
        assert!(cpus > 0);
        println!("Available CPUs: {}", cpus);
    }

    #[test]
    fn test_pin_to_cpu() {
        let result = pin_to_cpu(0);

        match result {
            Ok(_) => println!("Successfully pinned to CPU 0"),
            Err(e) => println!("CPU pinning failed (may be unsupported): {}", e),
        }
    }
}
