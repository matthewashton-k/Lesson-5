use std::{sync::{MutexGuard, Mutex, mpsc, Arc, atomic::{AtomicBool, Ordering}}, thread, fmt::Display, time::Duration};

use rand::{random_range, prelude::IndexedRandom};

fn main() {
    custom_lock2();
}

/// returning from a thread.
fn cloning() {
    let string = String::from("hello world");
    let mut cloned = string.clone();
    let thread = thread::spawn(move || {
        // cloned is moved into this thread due to the move keyword
        let altered = cloned.push_str("pushed from thread");
        cloned
    });
    
    let mut cloned = string.clone();
    let thread2 = thread::spawn(move || {
        // cloned is moved into this thread due to the move keyword
        let altered = cloned.push_str("pushed from thread2");
        cloned
    });
    let val: String = thread.join().unwrap();
    let val2: String = thread2.join().unwrap();
}

fn thread_channel() {
    // Create a channel
    let (tx, rx) = mpsc::channel();
    
    let tx_clone = tx.clone();
    // Spawn a thread that sends a message
    thread::spawn(move || {
        tx_clone.send("Hello from another thread!").unwrap();
    });
    
    let tx_clone = tx.clone();
    // Spawn a thread that sends a message
    thread::spawn(move || {
        tx_clone.send("Hello from another thread!").unwrap();
    });
    
    // Receive the message in the main thread
    let message = rx.recv().unwrap();
    let message = rx.recv().unwrap();
    println!("{}", message);
}

fn reference_counting() {
    let reference_counted = Arc::new(String::new());
    // Arc<T> Send T has to implement Send and Sync
    // clone is not doing a deep copy of the inner string
    let string_ref: Arc<String> = reference_counted.clone();
    let thread = thread::spawn(move || {
        println!("{string_ref}, rc: {}", Arc::<String>::strong_count(&string_ref));
        std::thread::sleep(Duration::from_secs(1));
    });
    
    std::thread::sleep(Duration::from_secs_f32(0.2));
    let string_ref: Arc<String> = reference_counted.clone();
    let thread2 = thread::spawn(move || {
        println!("{string_ref}, rc: {}", Arc::<String>::strong_count(&string_ref));
    });
    
    thread2.join().unwrap();
    thread.join().unwrap();
}

fn mutex_compile1() {
    // immutable variable
    let reference_counted = Arc::new(Mutex::new(String::new()));
     
    // notice how string_ref is immutable
    let string_ref: Arc<Mutex<String>> = reference_counted.clone();
    let thread = thread::spawn(move || {
        // mutating through an immutable reference?
        (string_ref.lock().unwrap()).push_str("fromthread1");
    });
    
    let string_ref: Arc<Mutex<String>> = reference_counted.clone();
    let thread2 = thread::spawn(move || {
        (string_ref.lock().unwrap()).push_str("fromthread1");
    });
    thread2.join().unwrap();
    thread.join().unwrap();
}


struct CustomLock {
    inner: String,
    guard: Arc<AtomicBool>
}

/// Works, no race conditions
fn custom_lock() {
    let guard = Arc::new(AtomicBool::new(false));
    let locked = Arc::new(CustomLock {
        inner: String::new(),
        guard
    });
    
    let mut handles = Vec::new();
    for i in 0..5000 {
        let locked = Arc::clone(&locked);
        let rand_wait: f64 = random_range(0.0..1.0);
        let handle = thread::spawn(move || {
            // tiny sleep to make race conditions more likely without locking
            thread::sleep(Duration::from_secs_f64(rand_wait)); 
            while let Err(_) = locked.guard.compare_exchange(false, true, 
                std::sync::atomic::Ordering::Acquire, 
                std::sync::atomic::Ordering::Relaxed) {
                // spin until lock is acquired
            }
            unsafe {
                let unlocked: &mut CustomLock = &mut *(Arc::as_ptr(&locked) as *mut CustomLock);
                unlocked.inner.push_str("a");
            }
            locked.guard.store(false, std::sync::atomic::Ordering::Relaxed);
        });
        handles.push(handle);
    }
    let len = handles.len();
    for h in handles {
        h.join();
    }
    assert_eq!(len, locked.inner.len());
}

/// Has race conditions
fn custom_lock2() {
    let guard = Arc::new(AtomicBool::new(false));
    let locked = Arc::new(CustomLock {
        inner: String::new(),
        guard
    });
    
    let mut handles = Vec::new();
    for i in 0..5000 {
        let locked = Arc::clone(&locked);
        let rand_wait: f64 = random_range(0.0..1.0);
        let handle = thread::spawn(move || {
            // little sleep to make race conditions more likely without locking
            thread::sleep(Duration::from_secs_f64(rand_wait)); 
            'lockloop: loop {
                let val = locked.guard.load(std::sync::atomic::Ordering::Acquire);
                if val == false { 
                    locked.guard.store(true, Ordering::Relaxed);
                    break 'lockloop;
                }
            }

            unsafe {
                let unlocked: &mut CustomLock = &mut *(Arc::as_ptr(&locked) as *mut CustomLock);
                unlocked.inner.push_str("a");
            }
            locked.guard.store(false, std::sync::atomic::Ordering::Release);
        });
        handles.push(handle);
    }
    let len = handles.len();
    for h in handles {
        h.join();
    }
    assert_eq!(len, locked.inner.len());
}
