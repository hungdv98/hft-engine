use crossbeam_utils::CachePadded;
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct SpscQueue<T> {
    head: CachePadded<AtomicUsize>,
    tail: CachePadded<AtomicUsize>,
    buffer: Box<[UnsafeCell<MaybeUninit<T>>]>,
    mask: usize,
}

unsafe impl<T: Send> Send for SpscQueue<T> {}
unsafe impl<T: Send> Sync for SpscQueue<T> {}

impl<T> SpscQueue<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be greater than 0");
        assert!(capacity.is_power_of_two(), "capacity must be a power of 2");

        let buffer = (0..capacity)
            .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        SpscQueue {
            head: CachePadded::new(AtomicUsize::new(0)),
            tail: CachePadded::new(AtomicUsize::new(0)),
            buffer,
            mask: capacity - 1,
        }
    }

    #[inline(always)]
    pub fn push(&self, value: T) -> Result<(), T> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        if tail.wrapping_sub(head) == self.buffer.len() {
            return Err(value);
        }

        unsafe {
            let slot = self.buffer.get_unchecked(tail & self.mask);
            (*slot.get()).write(value);
        }

        self.tail.store(tail.wrapping_add(1), Ordering::Release);

        Ok(())
    }

    #[inline(always)]
    pub fn pop(&self) -> Option<T> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);

        if head == tail {
            return None;
        }

        let value = unsafe {
            let slot = self.buffer.get_unchecked(head & self.mask);
            (*slot.get()).assume_init_read()
        };

        self.head.store(head.wrapping_add(1), Ordering::Release);

        Some(value)
    }

    #[inline]
    pub fn len(&self) -> usize {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);
        tail.wrapping_sub(head)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }
}

impl<T> Drop for SpscQueue<T> {
    fn drop(&mut self) {
        while self.pop().is_some() {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_push_pop() {
        let queue = SpscQueue::new(4);

        assert_eq!(queue.push(1), Ok(()));
        assert_eq!(queue.push(2), Ok(()));

        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.pop(), Some(2));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn test_full_queue() {
        let queue = SpscQueue::new(2);

        assert_eq!(queue.push(1), Ok(()));
        assert_eq!(queue.push(2), Ok(()));
        assert_eq!(queue.push(3), Err(3));

        assert_eq!(queue.pop(), Some(1));
        assert_eq!(queue.push(3), Ok(()));
    }

    #[test]
    fn test_wraparound() {
        let queue = SpscQueue::new(4);

        for i in 0..100 {
            assert_eq!(queue.push(i), Ok(()));
            assert_eq!(queue.pop(), Some(i));
        }
    }

    #[test]
    fn test_len() {
        let queue = SpscQueue::new(4);

        assert_eq!(queue.len(), 0);
        queue.push(1).unwrap();
        assert_eq!(queue.len(), 1);
        queue.push(2).unwrap();
        assert_eq!(queue.len(), 2);
        queue.pop();
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_multi_threaded() {
        let queue = Arc::new(SpscQueue::new(1024));
        let queue_clone = queue.clone();

        let producer = thread::spawn(move || {
            for i in 0..10000 {
                while queue_clone.push(i).is_err() {
                    std::hint::spin_loop();
                }
            }
        });

        let consumer = thread::spawn(move || {
            let mut received = Vec::new();
            while received.len() < 10000 {
                if let Some(val) = queue.pop() {
                    received.push(val);
                } else {
                    std::hint::spin_loop();
                }
            }
            received
        });

        producer.join().unwrap();
        let received = consumer.join().unwrap();

        assert_eq!(received.len(), 10000);
        for (i, &val) in received.iter().enumerate() {
            assert_eq!(val, i);
        }
    }

    #[test]
    #[should_panic(expected = "capacity must be a power of 2")]
    fn test_non_power_of_two() {
        let _queue: SpscQueue<i32> = SpscQueue::new(3);
    }
}
