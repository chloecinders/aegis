use std::collections::HashMap;
use std::hash::Hash;
use std::mem;

struct Slot<K, V> {
    key: K,
    value: V,
    prev: usize,
    next: usize,
}

pub struct Lru<K, V> {
    slots: Vec<Option<Slot<K, V>>>,
    free: Vec<usize>,
    index: HashMap<K, usize>,
    head: usize,
    tail: usize,
    capacity: usize,
}

impl<K: Eq + Hash + Clone, V> Lru<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            index: HashMap::new(),
            head: usize::MAX,
            tail: usize::MAX,
            capacity: capacity.max(1),
        }
    }

    pub fn peek(&self, key: &K) -> Option<&V> {
        let slot = *self.index.get(key)?;

        self.slots[slot].as_ref().map(|entry| &entry.value)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let slot = *self.index.get(key)?;

        self.detach(slot);
        self.attach(slot);

        self.slots[slot].as_mut().map(|entry| &mut entry.value)
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some(&slot) = self.index.get(&key) {
            let entry = self.slots[slot].as_mut()?;
            let previous = mem::replace(&mut entry.value, value);

            self.detach(slot);
            self.attach(slot);

            return Some(previous);
        }

        let slot = match self.free.pop() {
            Some(slot) => slot,
            None => {
                self.slots.push(None);
                self.slots.len() - 1
            }
        };

        self.slots[slot] = Some(Slot {
            key: key.clone(),
            value,
            prev: usize::MAX,
            next: usize::MAX,
        });
        self.index.insert(key, slot);
        self.attach(slot);

        if self.index.len() > self.capacity {
            self.evict();
        }

        None
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let slot = self.index.remove(key)?;

        self.detach(slot);
        self.free.push(slot);
        self.slots[slot].take().map(|entry| entry.value)
    }

    fn evict(&mut self) -> Option<(K, V)> {
        let slot = self.tail;

        if slot == usize::MAX {
            return None;
        }

        self.detach(slot);

        let removed = self.slots[slot].take()?;

        self.index.remove(&removed.key);
        self.free.push(slot);

        Some((removed.key, removed.value))
    }

    fn attach(&mut self, slot: usize) {
        let head = self.head;

        if let Some(entry) = self.slots[slot].as_mut() {
            entry.prev = usize::MAX;
            entry.next = head;
        }

        if let Some(Some(entry)) = self.slots.get_mut(head) {
            entry.prev = slot;
        }

        self.head = slot;

        if self.tail == usize::MAX {
            self.tail = slot;
        }
    }

    fn detach(&mut self, slot: usize) {
        let Some(Some(entry)) = self.slots.get(slot) else {
            return;
        };

        let (prev, next) = (entry.prev, entry.next);

        if let Some(Some(before)) = self.slots.get_mut(prev) {
            before.next = next;
        } else if self.head == slot {
            self.head = next;
        }

        if let Some(Some(after)) = self.slots.get_mut(next) {
            after.prev = prev;
        } else if self.tail == slot {
            self.tail = prev;
        }
    }
}
