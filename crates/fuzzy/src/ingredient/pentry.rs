use std::collections::BTreeSet;

use libafl_bolts::rands::Rand;

/// IngrediantPantry is a data container for values
/// that are used during fuzzing and mutation.
/// One use case is to store a set of possible/interesing
/// values for a specific field in a message call.
/// Type parameters:
/// - `R`: random number generator
/// - `T`: the type of the values
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(bound = "T: Ord + serde::Serialize + serde::de::DeserializeOwned")]
pub struct IngrediantPantry<R, T> {
    pub values: BTreeSet<T>,

    _phantom: std::marker::PhantomData<R>,
}

/// RandomGeneratable is a trait for types that can be randomly generated.
pub trait RandomlyGeneratable<R> {
    fn generate(rand: &mut R) -> Self;
}

impl<R: Rand, T: RandomlyGeneratable<R> + Ord + Clone> IngrediantPantry<R, T> {
    pub fn random_select(&self, rand: &mut R) -> &T {
        assert!(self.values.len() > 0, "empty pantry");
        let v = rand.choose(&self.values);
        v
    }

    pub fn random_generate(&mut self, rand: &mut R) -> &T {
        let v = T::generate(rand);
        self.values.insert(v.clone());
        return self.values.get(&v).expect("impossible: value not found");
    }

    pub fn insert(&mut self, v: T) {
        self.values.insert(v);
    }

    pub fn remove(&mut self, v: &T) {
        self.values.remove(v);
    }
}
