use std::ops::{Div, DivAssign, Mul, MulAssign};

use libsofl_core::engine::types::{U256, Uint};


#[derive(Debug, Clone, Default)]
pub struct HPMultipler {
    numerator: Vec<U256>,
    denominator: Vec<U256>,
}

impl From<U256> for HPMultipler {
    fn from(value: U256) -> Self {
        Self {
            numerator: vec![value],
            denominator: vec![],
        }
    }
}

impl Div<U256> for HPMultipler {
    type Output = Self;

    fn div(mut self, rhs: U256) -> Self::Output {
        self.denominator.push(rhs);
        self
    }
}

impl Div<HPMultipler> for HPMultipler {
    type Output = Self;

    fn div(mut self, rhs: HPMultipler) -> Self::Output {
        self.numerator.extend(rhs.denominator);
        self.denominator.extend(rhs.numerator);
        self
    }
}

impl DivAssign<U256> for HPMultipler {
    fn div_assign(&mut self, rhs: U256) {
        self.denominator.push(rhs);
    }
}

impl DivAssign<HPMultipler> for HPMultipler {
    fn div_assign(&mut self, rhs: HPMultipler) {
        self.numerator.extend(rhs.denominator);
        self.denominator.extend(rhs.numerator);
    }
}

impl Mul<U256> for HPMultipler {
    type Output = Self;

    fn mul(mut self, rhs: U256) -> Self::Output {
        self.numerator.push(rhs);
        self
    }
}

impl Mul<HPMultipler> for HPMultipler {
    type Output = Self;

    fn mul(mut self, rhs: HPMultipler) -> Self::Output {
        self.numerator.extend(rhs.numerator);
        self.denominator.extend(rhs.denominator);
        self
    }
}

impl MulAssign<U256> for HPMultipler {
    fn mul_assign(&mut self, rhs: U256) {
        self.numerator.push(rhs);
    }
}

impl MulAssign<HPMultipler> for HPMultipler {
    fn mul_assign(&mut self, rhs: HPMultipler) {
        self.numerator.extend(rhs.numerator);
        self.denominator.extend(rhs.denominator);
    }
}

impl<const BITS: usize, const LIMBS: usize> From<HPMultipler>
    for Uint<BITS, LIMBS>
{
    fn from(mut value: HPMultipler) -> Self {
        for numerator in value.numerator.iter_mut() {
            for denominator in value.denominator.iter_mut() {
                let gcd = numerator.gcd(*denominator);
                *numerator /= gcd;
                *denominator /= gcd;
            }
        }

        // sort and reverse
        value.numerator.sort();
        value.numerator.reverse();
        value.denominator.sort();

        let mut numerators: Vec<Uint<BITS, LIMBS>> = value
            .numerator
            .into_iter()
            .map(|num| Self::from_limbs_slice(num.into_limbs().as_slice()))
            .collect::<Vec<_>>();
        let mut denominators: Vec<Uint<BITS, LIMBS>> = value
            .denominator
            .into_iter()
            .map(|num| Self::from_limbs_slice(num.into_limbs().as_slice()))
            .collect::<Vec<_>>();

        let mut result = Uint::<BITS, LIMBS>::from(1);

        // dynamically calculate the result, to avoid overflow and precision loss
        while !numerators.is_empty() {
            let numerator = numerators[0];
            while numerator * result / numerator != result {
                // there is an overflow, let's try to reduce the result a bit
                // let's it implicitly panic if there is not enough denominators
                result /= denominators.remove(0);
            }
            result *= numerators.remove(0);
        }
        while !denominators.is_empty() {
            result /= denominators.remove(0);
        }

        result
    }
}

impl HPMultipler {
    pub fn new() -> Self {
        Self::default()
    }

    // power
    pub fn pow(self, exp: u64) -> Self {
        // quick pow algorithm
        let mut exp = exp;
        let mut base = self;
        let mut result = Self::default();
        while exp > 0 {
            if exp & 1 == 1 {
                result *= base.clone();
            }
            exp >>= 1;
            base *= base.clone();
        }

        result
    }

    // reciprocal
    pub fn reciprocal(self) -> Self {
        let mut result = Self::default();
        result /= self;
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UFixed256 {
    pub raw_value: U256,
    pub decimals: u8,
}

impl UFixed256 {
    pub fn new(decimals: u8) -> Self {
        if decimals > 80 {
            panic!("decimals must be <= 80")
        }
        Self {
            raw_value: U256::ZERO,
            decimals,
        }
    }
}

impl UFixed256 {
    pub fn denominator(&self) -> U256 {
        U256::from(2).pow(U256::from(self.decimals))
    }
}

pub fn approx_eq<const BITS: usize, const LIMBS: usize>(
    mut a: Uint<BITS, LIMBS>,
    mut b: Uint<BITS, LIMBS>,
    multipler: Option<u64>,
) -> bool {
    // we always have a < b
    if a > b {
        std::mem::swap(&mut a, &mut b);
    }
    let diff = b - a;

    // get the multipler
    let multipler: Uint<BITS, LIMBS> = match multipler {
        Some(m) => Uint::from(m),
        None => Uint::from(1_000_000u64),
    };

    // to avoid overflow, we will use multiple steps to calculate the result
    let a_reduced = a / multipler;

    // at this point, we know that floor(a / multipler) <= diff
    // but it may be due to the precision loss
    a_reduced >= diff
}
