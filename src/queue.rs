use std::collections::VecDeque;

pub struct Queue<T> {
    items: VecDeque<T>,
    len: usize,
}

impl<T> Queue<T> {
    pub fn new() -> Self {
        Self {
            items: VecDeque::new(),
            len: 0,
        }
    }

    pub fn push_front(&mut self, item: T) {
        self.items.push_front(item);
        self.len += 1;
    }

    pub fn push_back(&mut self, item: T) {
        self.items.push_back(item);
        self.len += 1;
    }

    pub fn pop_front(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop_front() {
            self.len -= 1;
            Some(item)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
