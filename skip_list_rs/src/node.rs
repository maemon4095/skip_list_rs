use std::{marker::PhantomData, ptr::NonNull};

pub struct MaybeNode<K: Ord, V> {
    ptr: *mut u8,
    marker: PhantomData<(K, V)>,
}

impl<K: Ord, V> Clone for MaybeNode<K, V> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr.clone(),
            marker: PhantomData,
        }
    }
}

impl<K: Ord, V> Copy for MaybeNode<K, V> {}

impl<K: Ord, V> MaybeNode<K, V> {
    pub fn null() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
            marker: PhantomData,
        }
    }

    pub fn take(self) -> Option<Node<K, V>> {
        if self.ptr.is_null() {
            return None;
        }

        Some(Node {
            ptr: unsafe { NonNull::new_unchecked(self.ptr) },
            marker: PhantomData,
        })
    }

    pub fn take_ref(&self) -> Option<&Node<K, V>> {
        if self.ptr.is_null() {
            return None;
        }

        unsafe { Some(std::mem::transmute(self)) }
    }
    pub fn take_mut(&mut self) -> Option<&mut Node<K, V>> {
        if self.ptr.is_null() {
            return None;
        }

        unsafe { Some(std::mem::transmute(self)) }
    }
}
// key: K + value: V + level: usize + nexts: [MaybeNode<K, V>]
pub struct Node<K: Ord, V> {
    ptr: NonNull<u8>,
    marker: PhantomData<(K, V)>,
}

impl<K: Ord, V> Clone for Node<K, V> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr.clone(),
            marker: PhantomData,
        }
    }
}

impl<K: Ord, V> Copy for Node<K, V> {}

impl<K: Ord, V> Node<K, V> {
    fn offset_of_value() -> usize {
        std::mem::size_of::<K>()
    }
    fn offset_of_level() -> usize {
        Self::offset_of_value() + std::mem::size_of::<V>()
    }
    fn offset_of_nexts() -> usize {
        Self::offset_of_level() + std::mem::size_of::<usize>()
    }

    fn calc_layout_and_offset(level: usize) -> (std::alloc::Layout, usize, usize, usize, usize) {
        let key_layout = std::alloc::Layout::new::<K>();
        let value_layout = std::alloc::Layout::new::<V>();
        let level_layout = std::alloc::Layout::new::<usize>();
        let nexts_layout = std::alloc::Layout::array::<Self>(level).unwrap();
        let (layout, value_offset) = key_layout.extend(value_layout).unwrap();
        let (layout, level_offset) = layout.extend(level_layout).unwrap();
        let (layout, nexts_offset) = layout.extend(nexts_layout).unwrap();
        (layout, 0, value_offset, level_offset, nexts_offset)
    }

    pub fn new(key: K, value: V, level: usize) -> Self {
        let (layout, key_offset, value_offset, level_offset, nexts_offset) =
            Self::calc_layout_and_offset(level);

        let ptr = unsafe { std::alloc::alloc(layout) };

        unsafe {
            ptr.add(key_offset).cast::<K>().write(key);
            ptr.add(value_offset).cast::<V>().write(value);
            ptr.add(level_offset).cast::<usize>().write(level);
            let ptr = ptr.add(nexts_offset).cast::<MaybeNode<K, V>>();
            for idx in 0..level {
                ptr.add(idx).write(MaybeNode::null())
            }
        }

        Self {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            marker: PhantomData,
        }
    }

    pub fn value_ptr(&self) -> *mut V {
        unsafe { self.ptr.as_ptr().add(Self::offset_of_value()).cast::<V>() }
    }

    pub fn value(&self) -> &V {
        unsafe { self.value_ptr().as_ref().unwrap() }
    }
    pub fn value_mut(&mut self) -> &mut V {
        unsafe { self.value_ptr().as_mut().unwrap() }
    }

    pub fn key_ptr(&self) -> *mut K {
        self.ptr.as_ptr().cast::<K>()
    }

    pub fn key(&self) -> &K {
        unsafe { self.key_ptr().as_ref().unwrap() }
    }
    pub fn key_mut(&mut self) -> &mut K {
        unsafe { self.key_ptr().as_mut().unwrap() }
    }

    pub fn level(&self) -> usize {
        unsafe {
            self.ptr
                .as_ptr()
                .add(Self::offset_of_level())
                .cast::<usize>()
                .read()
        }
    }

    pub fn nexts(&self) -> &[MaybeNode<K, V>] {
        unsafe {
            let ptr = self.ptr.as_ptr().add(Self::offset_of_nexts()).cast();
            let len = self.level();
            std::slice::from_raw_parts(ptr, len)
        }
    }

    pub fn nexts_mut(&mut self) -> &mut [MaybeNode<K, V>] {
        unsafe {
            let ptr = self.ptr.as_ptr().add(Self::offset_of_nexts()).cast();
            let len = self.level();
            std::slice::from_raw_parts_mut(ptr, len)
        }
    }

    pub fn dispose(mut self) -> (K, V) {
        let ptr: *mut K = self.key_mut();
        let key = unsafe { ptr.read() };
        let ptr: *mut V = self.value_mut();
        let val = unsafe { ptr.read() };
        let level = self.level();

        let (layout, _, _, _, _) = Self::calc_layout_and_offset(level);
        unsafe { std::alloc::dealloc(self.ptr.as_ptr(), layout) };

        (key, val)
    }
}

impl<K: Ord, V> Into<MaybeNode<K, V>> for Node<K, V> {
    fn into(self) -> MaybeNode<K, V> {
        MaybeNode {
            ptr: self.ptr.as_ptr(),
            marker: PhantomData,
        }
    }
}
