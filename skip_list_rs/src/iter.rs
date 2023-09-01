use std::{marker::PhantomData, mem::ManuallyDrop};

use crate::{node::MaybeNode, Generator, SkipList};

pub struct IntoIter<K: Ord, V> {
    pub(crate) node: MaybeNode<K, V>,
}

impl<K: Ord, V> IntoIter<K, V> {
    pub(crate) fn new<G: Generator<bool>>(list: SkipList<K, V, G>) -> Self {
        let mut me = ManuallyDrop::new(list);
        unsafe { std::ptr::drop_in_place(&mut me.nodes) };
        unsafe { std::ptr::drop_in_place(&mut me.gen) };

        let head = me.nodes[0];

        Self { node: head }
    }
}

impl<'a, K: Ord, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        let Some(node) = self.node.take() else {
            return None;
        };

        self.node = node.nexts()[0];

        let pair = node.dispose();
        Some(pair)
    }
}

pub struct Iter<'a, K: Ord + 'a, V: 'a> {
    pub(crate) node: MaybeNode<K, V>,
    pub(crate) marker: PhantomData<&'a ()>,
}

impl<'a, K: Ord + 'a, V: 'a> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let Some(node) = self.node.take() else {
            return None;
        };

        self.node = node.nexts()[0];

        unsafe {
            let key = node.key_ptr().as_ref().unwrap();
            let val = node.value_ptr().as_ref().unwrap();

            Some((key, val))
        }
    }
}

pub struct IterMut<'a, K: Ord, V> {
    pub(crate) node: MaybeNode<K, V>,
    pub(crate) marker: PhantomData<&'a ()>,
}

impl<'a, K: Ord + 'a, V: 'a> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        let Some(node) = self.node.take() else {
            return None;
        };

        self.node = node.nexts()[0];

        unsafe {
            let key = node.key_ptr().as_ref().unwrap();
            let val = node.value_ptr().as_mut().unwrap();

            Some((key, val))
        }
    }
}
