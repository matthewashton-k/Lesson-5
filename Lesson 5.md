## Introduction to Send and Sync
* Send is for passing ownership between threads
	* Send is automatically implemented
	* Most things will implement Send
* Sync is for sharing references between threads
	* Most things will implement Sync
	* If a type T is Sync, that means &T is Send.
		* Even though a variable of type T where T is Sync CANNOT be moved between threads, 
		* A variable of type &T where T is Sync CAN be moved between threads
	

##### Will it compile?

First look at the definition of spawn:

**References moved into F must be 'static, and every piece of data moved into the closure must be Send

```rust
pub fn spawn<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
```

Quickly explain dyn Trait objects

Yes:
```rust
fn main() {
    let send: Box<dyn Send+Display> =  Box::new("this is send but not sync");
    let thread = thread::spawn(move || {
        println!("{send}");
    });
    thread.join().unwrap();
}
```
* Why: send is moved into the closure, the requirement is that stuff captured by F must be Send
No:
```rust
fn main() {
    let sync: Box<dyn Sync+Display> = Box::new("this is sync but not send");
    let thread = thread::spawn(move || {
        println!("{sync}");
    });
    thread.join().unwrap();
}
```
* Why: the sync variable does not implement Send
* if T implements Sync, that means &T implements Send, meaning references to T can be moved in and out of threads
* We are moving sync itsself into the thread, and sync doesnt implement Send.

Yes:
```rust
fn main() {
    let sync: &(dyn Sync+Display) = &"this is sync but not send";
    let thread = thread::spawn(move || {
        println!("{sync}");
    });
    thread.join().unwrap();
}
```
* this time we are moving a &'static reference to something that implements sync
* T implements Sync if and only if &T is send.
	* &T is known to be Send and we are moving a value of type &T into the thread so it works out
* the sync variable is a  STATIC REFERENCE

No:
```rust
fn main() {
    let some_string = String::new();
    let string_ref: &(dyn Sync + Display) = &some_string;
    let thread = thread::spawn(move || {
        println!("{string_ref}");
    });
    thread.join().unwrap();
}
```
* string_ref is not STATIC
* F: FnOnce() -> T + Send + 'static,
	* bound is not satisfied

## Having to either move ownership into the thread or have all references be static is inconvenient

#### Solution option one: using tricks to get static references
```rust
// the C way to do it (just hoping that the reference will live long enough)
// this is bad
fn main() {
    let some_string = String::new();
    let string_ref: &'static (dyn Sync + Display) = unsafe {
        let non_static_ptr = (&some_string) as *const String;
        &*(non_static_ptr)
    };
    let thread = thread::spawn(move || {
        println!("{string_ref}");
    });
    thread.join().unwrap();
}
```
* the safe option:
```rust
fn main() {
    let some_string = String::new();
    let string_ref: &'static (dyn Sync + Display) =  {
        let boxed = Box::new(some_string);
        Box::leak(boxed)
    };
    let thread = thread::spawn(move || {
        println!("{string_ref}");
    });
    thread.join().unwrap();
}
```
*Cases where this is ok:*
* you only need to create one of the things you want to leak.
* the piece of memory you are leaking would exist until the end of the program's life anyways
	* i.e. it is some shared global state that needs to exist for the lifetime of the program

### Solution option 2: Just not bothering with shared memory and references

**Consider cloning before moving into a thread, then passing that cloned value out when the thread exits.
```rust
fn main() {
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
}```
* that compiles, and such a paradigm can be used if you feel like it, but usually real life is more complicated than just being able to clone something a bunch of times and then process the returned values in main

**A more flexible alternative: thread channels

* multiple producer single consumer
```rust
fn main() {
    // Create a channel
    let (tx, rx) = mpsc::channel();
    
    // Spawn a thread that sends a message
    thread::spawn(move || {
        tx.send("Hello from another thread!").unwrap();
    });
    
    // Receive the message in the main thread
    let message = rx.recv().unwrap();
    println!("{}", message);
}
```
* information sent through a channel is still constrained by Send and Sync traits
* tx is cheaply clonable
* move a cloned tx into every new thread, each thread can produce values
* main thread has rx and processes received values

**Multi producer multi consumer
* TODO

**When to use thread channels:
* better to use thread channels than reference counting and shared memory when possible because it is simpler and easier to understand
* when you need message passing
* when your process needs to aggregate values from a bunch of different threads and you only need one thread to combine all that information.
*  If I/O is your reason for needing concurrency
	* although async can be a better option for this if it is network IO
		* Producing a Stream from an Iterator
#### Gotchas of thread channels or cloning:
* thread channels have overhead
* if you have a case where you need a multi producer multi consumer it can be complicated to be cloning producers and consumers all over the place
* message passing doesn't work for everything
* backpressure



### Solution option 3: Shared ownership with reference counting

**Reference counting:
* Instead of calling drop when owner goes out of scope, add an additional check to only drop when all owners are out of scope.

```rust
fn main() {
    let reference_counted = Arc::new(String::new());
    
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
```
* both println are reading the same chunk of memory.
* clone() method is not doing a deep copy of the string.

**what is a strong and weak count?
* drop is called when strong count is 0
* weak count doesn't affect when value is dropped
* weak references are typically only used to prevent circular reference loops from fucking shit up
* Weak doesn't implement Deref, but Arc does
	* meaning you have to call upgrade on a Weak-ly cloned variable to access the inner value
	* upgrade may return None if the value has already been dropped
* If you aren't implementing linked lists you probably won't have to worry about this.

#### Gotchas of Reference counting
* has to be immutable 
* Arc has overhead
* For an Arc\<T\> to be passed between threads, T must be Send and Sync.
	* because internally Arc is passing references to each thread when you clone it


### Mitigating thread channel gotchas

**MPMC is complicated and has thread wake overhead:
* ArrayQueue in crossbeam.

**backpressure becomes a problem in channels:
* note that the standard library channel is unbounded
	* you can fill up your entire available memory if you aren't consuming as fast as you are producing.
* have some kind of limit for the number of enqueued items in a channel
	* crossbeam bounded channel
	* async bounded channel

### Mitigating Reference counting Gotchas

**Cant mutate the inner value
* Mutex
	* think twice before wrapping everything in an Arc\<Mutex\<\T>\>
**Overhead
* Overhead is not normally a problem 
	* if the cost of arc cloning really is a problem then you must be on embedded hardware or some shit

**Added complexity (biggest gotcha in my opinion)
* Gotta be sure there arent cyclical references



### Interior Mutability
* allows you to mutate some data through a shared "immutable" reference 

#### Mutex:
* lock
	* blocks until it can acquire the MutexGuard
	* prevents other threads from accessing data until the MutexGuard is dropped
* try_lock
	* non blocking version that will return Err if the guard is not immediately available

**Wrapping data in a Mutex is a way to turn some data that is Send but not Sync Syncable
* if T is Send, then Mutex\<T\> is Send and Sync

**Poisoning
* a mutex guard is poisoned when a thread panics with the guard in scope
* once poisoned, all other attempts to acquire the guard result in an error


#### RwLock:
* usually better than mutexes because there will be less blocking happening if you are reading more often than writing
* Allows multiple threads to read without locking, but when a write happens you must lock 
* RwLock doesn't allow for making types Sync if they werent already
	* this is because RwLock allows for multiple threads to be reading a piece of data at the same time, and internally that means passing references to T in and out of threads 
**read
* blocks if there are any writeguards in scope
* returns a readguard, and multiple read guards are allowed

**write
* blocks until there are no read or write guards in scope
* once acquired you can mutate the inner value

##### A few problems with mutexes to be aware about:
* starvation
* added complexity and opportunity for deadlocks



##### When locking is overkill, consider Atomics:
* for primitive types
* interior mutability for simpler types
* much faster than mutexes

**Ordering 
* "\[the C++11 atomics memory model] is defined in terms of madness-inducing causality graphs that require a full book to properly understand in a practical way."
* Relaxed
	* weakest ordering
	* used for loads or stores
* Acquire
	* used for loads
	* it will make subsequent operations be ordered 
* Release
	* used for stores
	* previous operations will be ordered

```rust
use std::{thread, sync::{atomic::{AtomicI32, AtomicBool}, Arc}, time::Duration};

struct CustomLock {
    inner: String,
    guard: Arc<AtomicBool>
}

fn main() {
    let guard = Arc::new(AtomicBool::new(false));
    let locked = Arc::new(CustomLock {
        inner: String::new(),
        guard
    });
    
    let mut handles = Vec::new();
    for i in 0..10000 {
        let locked = Arc::clone(&locked);
        let handle = thread::spawn(move || {
            // tiny sleep to make race conditions more likely without locking
            thread::sleep(Duration::from_secs_f64(0.001)); 
            while let Err(_) = locked.guard.compare_exchange(false, true, std::sync::atomic::Ordering::Acquire, std::sync::atomic::Ordering::Relaxed) {
                // spin until lock is acquired
            }
            unsafe {
                let unlocked: &mut CustomLock = &mut *(Arc::as_ptr(&locked) as *mut CustomLock);
                unlocked.inner.push_str(&i.to_string());
            }
            locked.guard.store(false, std::sync::atomic::Ordering::Release);
        });
        handles.push(handle);
    }
    for h in handles {
        h.join();
    }
    println!("{}", locked.inner);
}

```


#### Will it compile interior mutability round:

Will compile:
```rust
    //immutable variable
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
```

Will not compile:

```rust
    let rc = Arc::new(Mutex::new(String::new()));

    let string_ref: Arc<Mutex<String>> = rc.clone();
    let thread = thread::spawn(move || {
        drop(*(string_ref.lock().unwrap()));
    });
    thread.join().unwrap();
```