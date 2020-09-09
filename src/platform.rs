use std::mem;

#[derive(Debug)]
pub struct RecordingTimestamp(i64);

impl RecordingTimestamp {
    //
    // new
    //

    #[cfg(target_os = "windows")]
    #[inline]
    pub fn now() -> Self {
        // https://docs.microsoft.com/en-us/windows/win32/sysinfo/acquiring-high-resolution-time-stamps
        // https://docs.rs/winapi/0.3.9/winapi/um/profileapi/fn.QueryPerformanceCounter.html
        // https://www.youtube.com/watch?v=tAcUIEoy2Yk
        use winapi::{
            um::winnt::LARGE_INTEGER,
            um::profileapi::QueryPerformanceCounter,
        };
        let time = unsafe {
            let mut count: LARGE_INTEGER = mem::zeroed(); // TODO: Remove this initialization
            QueryPerformanceCounter(&mut count);
            mem::transmute::<_, i64>(count)
        };
        Self(time)
    }

    #[cfg(not(target_os = "windows"))]
    #[inline]
    pub fn now() -> i64 {
        (OffsetDateTime::now_utc() - OffsetDateTime::unix_epoch()).whole_nanoseconds() as i64
    }

    //
    // to_nanoseconds
    //

    #[cfg(target_os = "windows")]
    #[inline]
    pub fn to_nanoseconds(&self) -> i128 {
        use winapi::{
            um::winnt::LARGE_INTEGER,
            um::profileapi::QueryPerformanceFrequency,
        };
        let frequency = unsafe {
            let mut frequency: LARGE_INTEGER = mem::zeroed(); // TODO: Remove this initialization
            QueryPerformanceFrequency(&mut frequency);
            mem::transmute::<_, i64>(frequency)
        };
        1_000_000_000 * self.0 as i128 / frequency as i128
    }
    
    #[cfg(not(target_os = "windows"))]
    #[inline]
    pub fn to_nanoseconds(&self) -> i128 {
        self.0 as i128
    }
}
