#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
extern crate axstd as std;

// Use ax_println! from axlog instead of println! from axstd.
// axlog's print uses SpinNoIrq which disables interrupts while
// printing, preventing timer-interrupt-driven preemption from
// triggering inside a critical section.
#[cfg(feature = "axstd")]
#[macro_use]
extern crate axlog;

#[cfg(feature = "axstd")]
const LOOP_NUM: usize = 256;

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    #[cfg(feature = "axstd")]
    {
        use std::collections::VecDeque;
        use std::os::arceos::modules::axsync::spin::SpinNoIrq;
        use std::sync::Arc;
        use std::thread;

        ax_println!("Multi-task(Preemptible) is starting ...");

        let q1 = Arc::new(SpinNoIrq::new(VecDeque::new()));
        let q2 = q1.clone();

        // Worker1 (producer): pushes messages into the queue WITHOUT
        // calling yield_now(). It relies on the CFS scheduler's timer
        // interrupt to preempt it and give worker2 a chance to run.
        let worker1 = thread::spawn(move || {
            ax_println!("worker1 ... {:?}", thread::current().id());
            for i in 0..=LOOP_NUM {
                ax_println!("worker1 [{i}]");
                q1.lock().push_back(i);
            }
            ax_println!("worker1 ok!");
        });

        // Worker2 (consumer): pops messages from the queue. When the
        // queue is empty, it yields voluntarily.
        let worker2 = thread::spawn(move || {
            ax_println!("worker2 ... {:?}", thread::current().id());
            loop {
                if let Some(num) = q2.lock().pop_front() {
                    ax_println!("worker2 [{num}]");
                    if num == LOOP_NUM {
                        break;
                    }
                } else {
                    ax_println!("worker2: nothing to do!");
                    // TODO: it should sleep and wait for notify!
                    thread::yield_now();
                }
            }
            ax_println!("worker2 ok!");
        });

        ax_println!("Wait for workers to exit ...");
        let _ = worker1.join();
        let _ = worker2.join();

        ax_println!("Multi-task(Preemptible) ok!");
    }
    #[cfg(not(feature = "axstd"))]
    {
        println!("This application requires the 'axstd' feature for preemptive CFS scheduling.");
        println!("Run with: cargo xtask run [--arch <ARCH>]");
    }
}
