pub trait Generator<T> {
    fn gen(&mut self) -> T;
}

impl<T, F: FnMut() -> T> Generator<T> for F {
    fn gen(&mut self) -> T {
        self()
    }
}
