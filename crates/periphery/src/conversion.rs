use alloy_dyn_abi::DynSolValue;

pub trait PeripheryConvertTo<T> {
    fn cvt2(&self) -> T;
}

pub trait PeripheryConvertFrom<F> {
    fn cvt2(from: F) -> Self;
}

impl<T: PeripheryConvertTo<U>, U> PeripheryConvertFrom<T> for U {
    fn cvt2(from: T) -> Self {
        from.cvt2()
    }
}

impl<T> PeripheryConvertTo<DynSolValue> for Vec<T>
where
    T: Into<DynSolValue> + Copy,
{
    fn cvt2(&self) -> DynSolValue {
        DynSolValue::Array(self.iter().map(move |v| (*v).into()).collect())
    }
}
