mod recording;
mod platform;

#[cfg(test)] 
mod tests;

pub use recording::{recv, try_recv, RegionRecord, ThreadProfile, RegionExecution};

#[macro_export]
macro_rules! region {
    ($name: expr) => {
        $crate::recording::RegionRecord::new($name, file!(), line!())
    }
}
