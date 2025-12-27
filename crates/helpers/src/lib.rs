use std::cell::RefCell;
use std::hash::BuildHasherDefault;
use std::panic;
use std::sync::Once;

pub type IndexMap<K, V> = indexmap::IndexMap<K, V, BuildHasherDefault<ahash::AHasher>>;
pub type IndexSet<V> = indexmap::IndexSet<V, BuildHasherDefault<ahash::AHasher>>;

pub trait Config: Sized {
    fn config(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}

impl<T> Config for T {}

pub fn enter_panic(context: String) -> PanicContext {
    static ONCE: Once = Once::new();
    ONCE.call_once(PanicContext::init);

    with_ctx(|ctx| ctx.push(context));
    PanicContext { _priv: () }
}

#[must_use]
pub struct PanicContext {
    _priv: (),
}

impl PanicContext {
    #[allow(clippy::print_stderr)]
    fn init() {
        let default_hook = panic::take_hook();
        let hook = move |panic_info: &panic::PanicHookInfo<'_>| {
            with_ctx(|ctx| {
                if !ctx.is_empty() {
                    eprintln!("Panic context:");
                    for frame in ctx.iter() {
                        eprintln!("> {frame}\n");
                    }
                }
                default_hook(panic_info);
            });
        };
        panic::set_hook(Box::new(hook));
    }
}

impl Drop for PanicContext {
    fn drop(&mut self) {
        with_ctx(|ctx| assert!(ctx.pop().is_some()));
    }
}

fn with_ctx(f: impl FnOnce(&mut Vec<String>)) {
    thread_local! {
        static CTX: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }
    CTX.with(|ctx| f(&mut ctx.borrow_mut()));
}
