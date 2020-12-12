use cidr::Ipv4Cidr;

use bitstring_trees::set::RadixSet;

pub struct Cache {
    tree: RadixSet<Ipv4Cidr>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            tree: RadixSet::new(),
        }
    }
}
