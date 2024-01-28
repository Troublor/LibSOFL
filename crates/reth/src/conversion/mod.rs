pub mod reth;
pub mod uint;

pub trait ConvertTo<T> {
    fn cvt(self) -> T;
}

impl<T, C: ConvertTo<T> + Clone> ConvertTo<T> for &C {
    fn cvt(self) -> T {
        C::cvt(self.clone())
    }
}

pub trait ConvertFrom<F> {
    fn cvt(from: F) -> Self;
}

impl<T: ConvertTo<U>, U> ConvertFrom<T> for U {
    fn cvt(from: T) -> Self {
        from.cvt()
    }
}
