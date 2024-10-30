#[derive(Clone, Debug)]
pub struct CircularBuffer<T> {
    values: Vec<Option<T>>,
    size: u32,
    mask: u32,
}

impl<T: Clone> CircularBuffer<T> {
    pub fn new(initial_size: u32) -> Self {
        debug_assert!(initial_size.is_power_of_two());
        Self {
            values: vec![None; initial_size as usize],
            size: initial_size,
            mask: initial_size - 1,
        }
    }

    pub fn set(&mut self, seq: u32, val: T) {
        let idx = (seq & self.mask) as usize;

        match &self.values[idx] {
            None => {
                self.values[idx] = Some(val);
            }
            Some(_) => {
                // Need to grow the buffer
                let mut old_values = Vec::with_capacity(self.size as usize);
                std::mem::swap(&mut old_values, &mut self.values);

                while (seq & self.mask) == (seq & (self.size - 1)) {
                    self.size *= 2;
                    self.mask = self.size - 1;
                }

                self.values = vec![None; self.size as usize];
                self.values[(seq & self.mask) as usize] = Some(val);

                // Reinsert old values
                for (i, v) in old_values.into_iter().enumerate() {
                    if let Some(v) = v {
                        let new_idx = (i as u32 & self.mask) as usize;
                        self.values[new_idx] = Some(v);
                    }
                }
            }
        }
    }

    pub fn get(&self, seq: u32) -> Option<&T> {
        self.values[(seq & self.mask) as usize].as_ref()
    }

    pub fn remove(&mut self, seq: u32) -> Option<T> {
        let idx = (seq & self.mask) as usize;
        self.values[idx].take()
    }
}
