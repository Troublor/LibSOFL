use alloy_dyn_abi::{DynSolValue, JsonAbiExt};
use alloy_json_abi::Function;
use libsofl_core::{conversion::ConvertTo, engine::types::Bytes};

#[derive(Debug, Clone)]
pub enum StructuredCalldata {
    Typed(Function, Vec<DynSolValue>),
    Raw(Bytes),
}

impl Default for StructuredCalldata {
    fn default() -> Self {
        StructuredCalldata::Raw(Bytes::new())
    }
}

impl StructuredCalldata {
    pub fn bytes(&self) -> Bytes {
        match self {
            StructuredCalldata::Typed(func, args) => {
                let mut data =
                    func.abi_encode_input(args).expect("invalid input data");
                data.cvt()
            }
            StructuredCalldata::Raw(data) => data.clone(),
        }
    }
}

// TODO: implement mutation for TypedCalldata
