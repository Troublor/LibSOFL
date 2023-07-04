use std::ops::{Div, DivAssign, Mul, MulAssign};

use revm_primitives::{ruint::Uint, U256};

#[derive(Debug, Clone)]
pub struct HPMultipler {
    numerator: Vec<U256>,
    denominator: Vec<U256>,
}

impl Default for HPMultipler {
    fn default() -> Self {
        Self {
            numerator: vec![U256::from(1)],
            denominator: vec![U256::from(1)],
        }
    }
}

impl From<U256> for HPMultipler {
    fn from(value: U256) -> Self {
        Self {
            numerator: vec![value],
            denominator: vec![U256::from(1)],
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

        let mut result = Uint::<BITS, LIMBS>::from(1);
        for numerator in value.numerator.iter() {
            result *= Self::from_limbs_slice(numerator.into_limbs().as_slice());
        }
        for denominator in value.denominator.iter() {
            result /=
                Self::from_limbs_slice(denominator.into_limbs().as_slice());
        }

        result
    }
}
