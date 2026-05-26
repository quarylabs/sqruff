use sqruff_lib::templaters::TEMPLATERS;

pub(crate) fn templaters() {
    for templater in TEMPLATERS {
        println!("{}", templater.as_str());
    }
}
