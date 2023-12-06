pub trait Boxed {
    fn boxed(self) -> Box<Self>
    where
        Self: Sized;
}

impl<T> Boxed for T {
    fn boxed(self) -> Box<Self> {
        Box::new(self)
    }
}
