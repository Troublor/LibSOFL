use crate::engine::state::BcState;

pub mod corpus;
pub mod executor;
pub mod feedback;
pub mod generator;
pub mod observer;
pub mod state;

// Some common traits

pub trait OffersBcState<S: BcState<DBERR>, DBERR> {
    fn offer_bc_state(&self) -> &S;
}
