use std::{
    thread::{self, ThreadId},
    collections::{VecDeque, BTreeMap},
    cell::RefCell,
    sync::{Mutex, Condvar},
    rc::Rc,
};

use crate::platform::RecordingTimestamp;

use time::{Duration};
use lazy_static::lazy_static;

#[derive(Default)]
struct Global {
    pub profiles: VecDeque<Box<RawThreadProfile>>,
}

lazy_static! {
    static ref GLOBAL: Mutex<Global> = Mutex::new(Global::default());
    static ref CONDVAR: Condvar = Condvar::new();
} 

thread_local! {
    static THREAD_LOCAL: RefCell<ThreadLocal> = RefCell::new(ThreadLocal::new());
}

#[derive(Debug)]
pub struct RawThreadProfile {
    pub thread_id: ThreadId,
    pub region_backends: Vec<RegionRecordBackend>,
}

impl RawThreadProfile {
    fn new(thread_id: ThreadId) -> Self {
        Self {
            thread_id,
            region_backends: Vec::with_capacity(1024),
        }
    }
}

#[derive(Debug)]
struct ThreadLocal {
    raw_thread_profile: Option<Box<RawThreadProfile>>,
    stack: VecDeque<usize>,
}

impl ThreadLocal {
    pub fn new() -> Self {
        Self {
            raw_thread_profile: Some(Box::new(RawThreadProfile::new(thread::current().id()))),
            stack: VecDeque::new(),
        }
    }
}

pub struct RegionRecord;

impl RegionRecord {
    #[inline]
    pub fn new(name: &'static str, file: &'static str, line: u32) -> Self {
        THREAD_LOCAL.with(|thread_local| {
            let mut thread_local = thread_local.borrow_mut();

            // Get the next index and initialize the RawThreadProfile
            let current_index = if let Some(ref mut raw_thread_profile) = thread_local.raw_thread_profile {
                raw_thread_profile.region_backends.len()
            } else {
                thread_local.raw_thread_profile = Some(Box::new(RawThreadProfile::new(thread::current().id())));
                0
            };

            // Get the index of the parent and push the next index
            let parent = if thread_local.stack.len() > 0 {
                thread_local.stack.get(thread_local.stack.len() - 1).cloned()
            } else {
                None
            };
            thread_local.stack.push_back(current_index);

            // Insert the actual information into the profile
            if let Some(ref mut raw_thread_profile) = thread_local.raw_thread_profile {
                raw_thread_profile.region_backends.push(RegionRecordBackend {
                    name,
                    file,
                    line,
                    parent,
                    start: RecordingTimestamp::now(),
                    end: None,
                });    
            }
        });
        RegionRecord
    }
}

impl Drop for RegionRecord {
    #[inline]
    fn drop(&mut self) {
        let time = RecordingTimestamp::now();
        THREAD_LOCAL.with(|thread_local| {
            let mut thread_local = thread_local.borrow_mut();
            debug_assert!(!thread_local.stack.is_empty(), "the stack of the thread profile must not be empty");

            // Add the end time to the region
            let index = *thread_local.stack.back().expect("stack is not empty");
            if let Some(ref mut raw_thread_profile) = thread_local.raw_thread_profile {
                raw_thread_profile.region_backends[index].end = Some(time);
                thread_local.stack.pop_back();
            } else {
                panic!("the RawThreadProfile must be initialized when drop is called");
            }

            // If the stack is empty the regions can be send to the global collection point
            if thread_local.stack.len() == 0 {
                let mut g = GLOBAL.lock().unwrap();
                let raw_thread_profile = thread_local.raw_thread_profile
                    .take()
                    .expect("there must be a raw_thread_profile when drop is executed");
                g.profiles.push_back(raw_thread_profile);
                drop(g);
                CONDVAR.notify_one();
            }
        });
    }
}

#[derive(Debug)]
pub struct RegionRecordBackend {
    pub name: &'static str,
    pub file: &'static str,
    pub line: u32,
    pub parent: Option<usize>,
    pub start: RecordingTimestamp,
    pub end: Option<RecordingTimestamp>,
}

pub fn recv() -> Box<RawThreadProfile> {
    let mut g = GLOBAL.lock().unwrap();
    while g.profiles.is_empty() {
        g = CONDVAR.wait(g).unwrap();
    }
    g.profiles.pop_front().expect("is_empty equals false")
}

pub fn try_recv() -> Option<Box<RawThreadProfile>> {
    let mut g = GLOBAL.lock().unwrap();
    g.profiles.pop_front()
}



#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Region {
    pub name: &'static str,
    pub file: &'static str,
    pub line: u32,
}

impl From<&RegionRecordBackend> for Region {
    fn from(region_record_backend: &RegionRecordBackend) -> Self {
        Region {
            name: region_record_backend.name,
            file: region_record_backend.file,
            line: region_record_backend.line,
        }
    }
}




#[derive(Debug, Clone)]
pub struct Instant {
    nanoseconds: i128,
}

impl Instant {
    fn new(nanoseconds: i128) -> Self {
        Self {
            nanoseconds,
        }
    }

    fn as_nanoseconds(&self) -> f64 {
        self.nanoseconds as f64
    }

    fn as_microseconds(&self) -> f64 {
        self.as_nanoseconds() / 1_000.0
    }

    fn as_milliseconds(&self) -> f64 {
        self.as_nanoseconds() / 1_000_000.0
    }
}

#[derive(Debug, Clone)]
pub struct RegionExecution {
    pub children: Vec<RegionExecution>,
    pub region: Rc<Region>,
    pub start: Instant,
    pub end: Instant,
}

impl RegionExecution {
    pub fn duration(&self) -> Duration {
        Duration::nanoseconds(self.end.nanoseconds as i64) - Duration::nanoseconds(self.start.nanoseconds as i64)
    }
}

#[derive(Debug)]
pub struct ThreadProfile {
    pub regions: BTreeMap<Rc<Region>, Vec<RegionExecution>>,
    pub root_region_executions: Vec<RegionExecution>,
}

impl ThreadProfile {
    pub fn to_chrome_tracing(&self) -> String {
        let mut trace_events = json::Array::new();
        fn traverse(region_execution: &RegionExecution, trace_events: &mut json::Array, pid: u32) {            
            let start = region_execution.start.nanoseconds as f64 / 1000.0;
            let duration = (region_execution.end.nanoseconds - region_execution.start.nanoseconds) as f64 / 1000.0;
            let data = json::object! {
                name: region_execution.region.name,
                ph: "X",
                ts: start,
                dur: duration,
                pid: pid,
            };
            trace_events.push(data);
            for child_region_execution in &region_execution.children {
                traverse(child_region_execution, trace_events, pid);
            }
        }
        let pid = std::process::id();
        for region_execution in &self.root_region_executions {
            traverse(region_execution, &mut trace_events, pid);
        }
        json::stringify(trace_events)
    }
}

impl RawThreadProfile {
    pub fn profile(&self) -> ThreadProfile {
        let mut root_region_execution_indices = Vec::new();
        let mut children_indices: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (index, region_backend) in self.region_backends.iter().enumerate() {
            if let Some(parent_index) = region_backend.parent {
                children_indices.entry(parent_index)
                    .and_modify(|children| children.push(index))
                    .or_insert(vec![index]);
            } else {
                root_region_execution_indices.push(index);
            }
        }

        let mut regions: BTreeMap<Rc<Region>, Vec<RegionExecution>> = BTreeMap::new();
        let mut root_region_executions = Vec::new();
        for root_region_index in &root_region_execution_indices {
            let region_backend = &self.region_backends[*root_region_index];
            let root_region = self.generate_region_execution(region_backend, *root_region_index, &children_indices, &mut regions);
            root_region_executions.push(root_region);
        }
        ThreadProfile {
            regions,
            root_region_executions,
        }
    }

    fn generate_region_execution(
        &self,
        region_backend: &RegionRecordBackend, 
        index: usize, 
        children_indices: &BTreeMap<usize, Vec<usize>>, 
        regions: &mut BTreeMap<Rc<Region>, Vec<RegionExecution>>
    ) -> RegionExecution {
        let mut children = Vec::new();
        // Traverse the tree of RegionRecordBackends and collect the children for this one
        if let Some(indices) = children_indices.get(&index) {
            for child_index in indices {
                let child_region_backend = &self.region_backends[*child_index];
                let child = self.generate_region_execution(child_region_backend, *child_index, children_indices, regions);
                children.push(child);
            }
        }
        // Generate the Region and RegionExecution for this RegionRecordBackend
        let region = Rc::new(Region::from(region_backend));
        let region_execution = RegionExecution {
            children: children.clone(),
            region: region.clone(),
            start: Instant::new(region_backend.start.to_nanoseconds()),
            end: Instant::new(region_backend.end.as_ref().expect("The RegionBackend must be finalized by setting the end time before.").to_nanoseconds()),
        };
        // Add the children to the profile-wide mapping from Region to RegionExecution
        regions.entry(region)
            .and_modify(|region_executions| region_executions.push(region_execution.clone()))
            .or_insert(vec![region_execution.clone()]);
        region_execution
    }
}
