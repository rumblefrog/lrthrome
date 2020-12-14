use std::net::Ipv4Addr;

use cidr::{Cidr, Ipv4Cidr};

use crate::error::LrthromeResult;
use crate::sources::Sources;

pub struct Cache {
    tree: PrefixTree,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            tree: PrefixTree::new(),
        }
    }

    pub fn exist(&self, addr: Ipv4Addr) -> bool {
        self.tree.contains_addr(addr)
    }

    pub async fn temper(&mut self, sources: &Sources) -> LrthromeResult<()> {
        for source in sources.sources() {
            let iter = source.iterate_cidr().await?;

            for cidr in iter {
                self.tree.add_cidr(&cidr);
            }
        }

        Ok(())
    }
}

type Branch<T> = Option<Box<Node<T>>>;

#[derive(Clone, Debug)]
pub struct Node<T>
where
    T: Clone,
{
    left: Branch<T>,

    right: Branch<T>,

    value: Option<T>,
}

impl<T: Clone> Node<T> {
    fn new() -> Node<T> {
        Node::<T> {
            left: None,
            right: None,
            value: None,
        }
    }

    fn insert(&mut self, key: u32, mask: u32, value: T) {
        let bit: u32 = 0x8000_0000;
        if mask == 0 {
            self.value = Some(value);
            return;
        }
        let next_node = if (key & bit) == 0 {
            &mut self.left
        } else {
            &mut self.right
        };
        match *next_node {
            Some(ref mut boxed_node) => boxed_node.insert(key << 1, mask << 1, value),
            None => {
                let mut new_node = Node::<T> {
                    value: None,
                    left: None,
                    right: None,
                };
                new_node.insert(key << 1, mask << 1, value);
                *next_node = Some(Box::new(new_node));
            }
        }
    }

    fn _find(&self, key: u32, mask: u32, cur_val: Option<T>) -> Option<T> {
        let bit: u32 = 0x8000_0000;
        if mask == 0 {
            return self.value.clone().or(cur_val);
        }

        let next_node = if (key & bit) == 0 {
            &self.left
        } else {
            &self.right
        };
        match *next_node {
            Some(ref boxed_node) => {
                boxed_node._find(key << 1, mask << 1, self.value.clone().or(cur_val))
            }
            None => self.value.clone().or(cur_val),
        }
    }

    fn find(&self, key: u32, mask: u32) -> Option<T> {
        self._find(key, mask, None)
    }
}

#[derive(Debug)]
pub struct PrefixTree {
    root: Node<u8>,
}

impl PrefixTree {
    pub fn new() -> PrefixTree {
        PrefixTree {
            root: Node::<u8>::new(),
        }
    }

    pub fn add_cidr(&mut self, cidr: &Ipv4Cidr) {
        self.root
            .insert(u32::from(cidr.first_address()), u32::from(cidr.mask()), 1);
    }

    pub fn contains_addr(&self, addr: Ipv4Addr) -> bool {
        self.root.find(u32::from(addr), 0xffff_ffff).is_some()
    }
}
