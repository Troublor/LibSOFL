pub mod revm;
pub mod uint;
pub trait ConvertTo<T> {
    fn cvt(&self) -> T;
}

pub trait ConvertFrom<F> {
    fn cvt(from: F) -> Self;
}

impl<T: ConvertTo<U>, U> ConvertFrom<T> for U {
    fn cvt(from: T) -> Self {
        from.cvt()
    }
}