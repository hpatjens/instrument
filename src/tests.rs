mod profiling {
    use crate as instrument;

    use std::{sync::Mutex, time::Duration, thread};
    use lazy_static::lazy_static;
    use serial_test::serial;

    lazy_static! {
        static ref TESTING: Mutex<()> = Mutex::new(());
    }

    const SLEEP_DURATION_MILLIS: i64 = 20;
    const SLEEP_DURATION_NANOS: i64 = SLEEP_DURATION_MILLIS * 1_000_000;
    const DURATION_ACCURACY_NANOS: i64 = 2_000_000;

    fn sleep() {
        thread::sleep(Duration::from_millis(SLEEP_DURATION_MILLIS as u64));
    }
    
    #[test]
    #[serial]
    fn single_thread_single_region() {
        fn main() { 
            let _region = instrument::region!("main");
            sleep();
        }
        main();
        let result = instrument::recv();
        assert_eq!(result.region_backends.len(), 1);
        assert!(instrument::try_recv().is_none());
        let profile = result.profile();
        assert_eq!(profile.root_region_executions.len(), 1);
    }

    #[test]
    #[serial]
    fn single_thread_multiple_regions() {
        fn function1() {
            let _region = instrument::region!("function1");
            sleep();
        }
        fn main() { 
            let _region = instrument::region!("main");
            function1();
        }
        main();
        let raw_thread_profile = instrument::recv();
        assert_eq!(raw_thread_profile.region_backends.len(), 2);
        let thread_profile = raw_thread_profile.profile();
        assert_eq!(thread_profile.regions.len(), 2);
        assert_eq!(thread_profile.root_region_executions.len(), 1);
        let ref root = thread_profile.root_region_executions[0];
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.region.name, "main");
        assert_eq!(root.region.file, file!());
        assert_eq!(root.region.line, line!() - 13);
        assert!(i128::abs(root.duration().whole_nanoseconds() - SLEEP_DURATION_NANOS as i128) < DURATION_ACCURACY_NANOS as i128);
        assert_eq!(thread_profile.root_region_executions[0].children.len(), 1);
        assert!(instrument::try_recv().is_none());
    }

    #[test]
    #[serial]
    fn multi_thread_single_regions() {
        let join_handle1 = thread::spawn(|| {
            fn main1() {
                let _region = instrument::region!("main1");
                sleep();
            }
            main1()
        });
        let join_handle2 = thread::spawn(|| {
            fn main2() {
                let _region = instrument::region!("main2");
                sleep();
            }
            main2()
        });
        join_handle1.join().unwrap();
        join_handle2.join().unwrap();
        let raw_thread_profile_a = instrument::recv(); // We don't know which RawThreadProfile is returned first
        let raw_thread_profile_b = instrument::recv();
        assert_eq!(raw_thread_profile_a.region_backends.len(), 1);
        assert_eq!(raw_thread_profile_b.region_backends.len(), 1);
        let thread_profile_a = raw_thread_profile_a.profile();
        let thread_profile_b = raw_thread_profile_b.profile();
        assert_eq!(thread_profile_a.regions.len(), 1);
        assert_eq!(thread_profile_b.regions.len(), 1);
        assert!(
            thread_profile_a.root_region_executions[0].region.name == "main1" && thread_profile_b.root_region_executions[0].region.name == "main2" ||
            thread_profile_b.root_region_executions[0].region.name == "main1" && thread_profile_a.root_region_executions[0].region.name == "main2"
        );
        assert!(instrument::try_recv().is_none());
    }

    #[test]
    #[serial]
    fn single_thread_multiple_regions_measurement() {
        const COUNT: usize = 2_000;
        fn function1() {
            let _region = instrument::region!("function1");
            thread::sleep(Duration::from_millis(1));
            
        }
        fn main() { 
            let _region = instrument::region!("main");
            for _ in 0..COUNT {
                function1();
            }
        }
        main();
        let result = instrument::recv();
        assert!(instrument::try_recv().is_none());
        let profile = result.profile();
        assert_eq!(profile.root_region_executions.len(), 1);
    }
}
