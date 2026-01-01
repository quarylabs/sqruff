use sqruff_lib_core::dialects::init::dialect_readout;

pub(crate) fn dialects() {
    for dialect in dialect_readout() {
        println!("{}", dialect);
    }
}
