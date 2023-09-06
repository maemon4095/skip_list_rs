mod generator;
mod iter;
mod node;
pub use generator::Generator;
use iter::{IntoIter, Iter, IterMut};
use node::{MaybeNode, Node};
use std::{iter::repeat, marker::PhantomData};
pub struct SkipList<K: Ord, V, G: Generator<bool>> {
    gen: G,
    count: usize,
    nodes: Vec<MaybeNode<K, V>>,
}

impl<K: Ord, V, G: Generator<bool>> SkipList<K, V, G> {
    pub fn new(gen: G) -> Self {
        Self {
            gen,
            count: 0,
            nodes: vec![MaybeNode::null()],
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn insert(&mut self, key: K, value: V) -> Result<(), (K, V)> {
        let len = self.nodes.len();
        let level = len - 1;
        let forwards = unsafe { std::slice::from_raw_parts_mut(self.nodes.as_mut_ptr(), len) };
        let inserted = self.insert_impl(forwards, level, key, value)?;

        if let Some(d) = inserted.level().checked_sub(len) {
            self.nodes
                .extend(repeat::<MaybeNode<K, V>>(inserted.into()).take(d));
        }

        Ok(())
    }

    // levelごとに再帰を行う．
    // 各levelで前方に進められるだけ進め，進められなくなればlevelを下げて再帰．
    // 巻き上げにおいて，forwardsの該当levelを挿入された要素にする．ただし，挿入されたnodeのlevelを超えた場合は何もしない．
    fn insert_impl(
        &mut self,
        mut forwards: &mut [MaybeNode<K, V>],
        level: usize,
        key: K,
        value: V,
    ) -> Result<Node<K, V>, (K, V)> {
        loop {
            //前方に進める．
            assert!(level < forwards.len());

            let Some(next) = forwards[level].take() else {
                break;
            };

            if next.key() == &key {
                return Err((key, value));
            }

            if next.key() > &key {
                break;
            }

            forwards = next.nexts_mut();
        }

        let node = if level == 0 {
            let n = self.alloc(key, value);
            self.count += 1;
            n
        } else {
            self.insert_impl(forwards, level - 1, key, value)?
        };

        if level >= node.level() {
            return Ok(node);
        }

        node.nexts_mut()[level] = forwards[level];
        forwards[level] = node.into();

        return Ok(node);
    }

    pub fn search(&self, key: &K) -> Option<&V> {
        let mut forwards = self.nodes.as_slice();

        for level in (0..forwards.len()).rev() {
            loop {
                let Some(next) = forwards.get(level).and_then(|e| e.take()) else {
                    break;
                };
                if next.key() >= key {
                    break;
                }
                forwards = next.nexts();
            }
        }

        let Some(node) = forwards.get(0).and_then(|e| e.take()) else {
            return None;
        };

        if node.key() == key {
            Some(node.value())
        } else {
            None
        }
    }

    pub fn remove(&mut self, key: &K) -> Result<(K, V), ()> {
        let len = self.nodes.len();
        let level = len - 1;
        let forwards = unsafe { std::slice::from_raw_parts_mut(self.nodes.as_mut_ptr(), len) };
        let removed = self.remove_impl(forwards, level, key)?;
        Ok(removed.dispose())
    }

    fn remove_impl(
        &mut self,
        mut forwards: &mut [MaybeNode<K, V>],
        level: usize,
        key: &K,
    ) -> Result<Node<K, V>, ()> {
        loop {
            //前方に進める．
            assert!(level < forwards.len());

            let Some(next) = forwards[level].take() else {
                break;
            };

            if next.key() >= &key {
                break;
            }

            forwards = next.nexts_mut();
        }

        let removed = if level == 0 {
            let Some(node) = forwards[level].take() else {
                return Err(());
            };
            self.count -= 1;
            node
        } else {
            self.remove_impl(forwards, level - 1, key)?
        };

        if level >= removed.level() {
            return Ok(removed);
        }

        let next = &mut removed.nexts_mut()[level];
        forwards[level] = *next;
        *next = MaybeNode::null();

        return Ok(removed);
    }

    fn alloc(&mut self, key: K, value: V) -> Node<K, V> {
        let level = {
            let limit = (usize::BITS - self.count.leading_zeros()) as usize;
            let mut size = 1;

            while size < limit && self.gen.gen() {
                size += 1;
            }
            size
        };

        Node::new(key, value, level)
    }

    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            node: self.nodes[0],
            marker: PhantomData,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut {
            node: self.nodes[0],
            marker: PhantomData,
        }
    }

    pub fn into_iter(self) -> IntoIter<K, V> {
        IntoIter::new(self)
    }
}

impl<K: Ord, V, R: Generator<bool>> Drop for SkipList<K, V, R> {
    fn drop(&mut self) {
        let nodes = &mut self.nodes;
        loop {
            let Some(next) = nodes[0].take() else {
                break;
            };

            nodes.clear();
            nodes.extend_from_slice(next.nexts());

            next.dispose();
        }
    }
}

impl<K: Ord, V, R: Generator<bool>> IntoIterator for SkipList<K, V, R> {
    type Item = (K, V);

    type IntoIter = IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_iter()
    }
}

#[cfg(test)]
mod test {
    use crate::node::Node;
    use crate::{Generator, SkipList};
    use mockalloc::Mockalloc;
    use rand::distributions::Distribution;
    use rand::distributions::Standard;
    use rand::rngs::SmallRng;
    use rand::SeedableRng;
    use std::alloc::System;
    use std::fmt::Debug;
    use std::marker::PhantomData;

    #[global_allocator]
    static ALLOCATOR: Mockalloc<System> = Mockalloc(System);

    #[mockalloc::test]
    fn test() {
        use rand::seq::SliceRandom;
        let mut rng = SmallRng::from_entropy();
        let gen = Gen::standard(SmallRng::from_entropy());
        let mut list = SkipList::new(gen);

        {
            println!("\n----- insert -----\n");
            let mut items: Vec<_> = (0..9).collect();
            items.shuffle(&mut rng);
            debug(&list);
            for item in items.iter().copied() {
                let result = list.insert(item, item);
                println!("insert({}) = {:?}", item, result);
                debug(&list);
            }
        }

        {
            println!("\n----- search -----\n");
            let mut keys: Vec<_> = (0..10).collect();
            keys.shuffle(&mut rng);
            for key in keys.iter().copied() {
                let result = list.search(&key);

                println!("search({}) = {:?}", key, result);
            }
        }

        {
            println!("\n----- remove -----\n");
            let mut keys: Vec<_> = (0..10).collect();
            keys.shuffle(&mut rng);
            for key in keys.iter().copied() {
                let result = list.remove(&key);
                println!("remove({}) = {:?}", key, result);
                debug(&list);
            }
        }

        {
            println!("\n----- search -----\n");
            let mut keys: Vec<_> = (0..10).collect();
            keys.shuffle(&mut rng);
            for key in keys.iter().copied() {
                let result = list.search(&key);

                println!("search({}) = {:?}", key, result);
            }
        }
    }

    #[mockalloc::test]
    fn iter() {
        use rand::seq::SliceRandom;
        let mut rng = SmallRng::from_entropy();
        let gen = Gen::standard(SmallRng::from_entropy());
        let mut list = SkipList::new(gen);

        println!("\n----- insert -----\n");
        let mut items: Vec<_> = (0..9).collect();
        items.shuffle(&mut rng);
        for item in items.iter().copied() {
            let result = list.insert(item, item);
            println!("insert({}) = {:?}", item, result);

            println!("{:?}", list.iter().collect::<Vec<_>>());
        }
    }

    fn debug<K: Ord + Debug, V, R: Generator<bool>>(list: &SkipList<K, V, R>) {
        use std::fmt::Write;
        use std::iter::{repeat, repeat_with};
        let mut forwards = list.nodes.as_slice();
        let mut lines: Vec<_> = repeat_with(String::new).take(forwards.len()).collect();
        let mut baseline = String::new();
        let mut current_node: Option<Node<K, V>> = None;
        let align = 10;
        loop {
            match current_node {
                Some(n) => write!(baseline, "{:<align$}", format!("{:?}", n.key())).unwrap(),
                None => baseline.extend(repeat(' ').take(align)),
            }

            for (no, node) in forwards.iter().enumerate() {
                write!(
                    lines[no],
                    "{:<align$}",
                    format!("{:?}", node.take().map(|e| e.key()))
                )
                .unwrap();
            }

            for ln in forwards.len()..lines.len() {
                lines[ln].extend(repeat(' ').take(align));
            }

            let Some(next) = forwards[0].take() else {
                break;
            };
            current_node = Some(next);
            forwards = next.nexts();
        }

        println!("┌{:─<x$}┐", "", x = baseline.len());
        for line in lines.iter().rev() {
            println!("│{}│", line);
        }
        println!("├{:─<x$}┤", "", x = baseline.len());
        println!("│{}│", baseline);
        println!("└{:─<x$}┘", "", x = baseline.len());
    }

    struct Gen<T, R: rand::Rng, D: Distribution<T>> {
        rng: R,
        distr: D,
        marker: PhantomData<T>,
    }

    impl<T, R: rand::Rng> Gen<T, R, Standard>
    where
        Standard: Distribution<T>,
    {
        fn standard(rng: R) -> Self {
            Gen {
                rng,
                distr: Standard,
                marker: PhantomData,
            }
        }
    }

    impl<T, R: rand::Rng, D: Distribution<T>> Generator<T> for Gen<T, R, D> {
        fn gen(&mut self) -> T {
            self.distr.sample(&mut self.rng)
        }
    }
}
