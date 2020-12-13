use cidr::Ipv4Cidr;

use bitstring_trees::set::RadixSet;

use crate::error::LrthromeResult;
use crate::sources::Sources;

pub struct Cache {
    tree: RadixSet<Ipv4Cidr>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            tree: RadixSet::new(),
        }
    }

    pub async fn temper(&mut self, sources: &Sources) -> LrthromeResult<()> {
        for source in sources.sources() {
            let mut iter = source.iterate_cidr().await?;

            for cidr in iter.next() {
                self.tree.insert(cidr);
            }
        }

        Ok(())
    }
}
