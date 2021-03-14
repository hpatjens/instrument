pub mod recording;
mod platform;

#[cfg(test)] 
mod tests;

pub use recording::{recv, try_recv, RegionRecord, ThreadProfile, RegionExecution, ToChromeTracing};

#[macro_export]
macro_rules! region {
    ($name: expr) => {
        let _region = $crate::recording::RegionRecord::new($name, file!(), line!());
    }
}
