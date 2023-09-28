use libafl::prelude::{
    DifferentialObserver, Observer, ObserversTuple, UsesInput,
};
use libafl_bolts::Named;
use revm::Database;

use crate::engine::inspectors::{
    asset_flow::{AssetFlowInspector, AssetTransfer},
    InspectorTuple, NoInspector,
};

use super::{DifferentialEvmObserver, EvmObserver};

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct AssetFlowObserver {
    pub flows: Vec<AssetTransfer>,
}

impl Named for AssetFlowObserver {
    fn name(&self) -> &str {
        "AssetFlowObserver"
    }
}

impl<S: UsesInput> Observer<S> for AssetFlowObserver {}

impl<S: UsesInput, BS: Database> EvmObserver<S, BS> for AssetFlowObserver {
    type Inspector = AssetFlowInspector;

    fn get_inspector(
        &mut self,
        _pre_state: &BS,
        _input: &<S as UsesInput>::Input,
    ) -> Result<Self::Inspector, libafl::Error> {
        Ok(AssetFlowInspector::new())
    }

    fn on_executed(
        &mut self,
        _post_state: &BS,
        _inspector: Self::Inspector,
        _results: Vec<revm_primitives::ExecutionResult>,
        _input: &<S as UsesInput>::Input,
    ) -> Result<(), libafl::Error> {
        for mut transfers in _inspector.transfers.into_iter() {
            self.flows.append(&mut transfers);
        }
        Ok(())
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct DifferentialAssetFlowObserver {
    pub first_flows: Vec<AssetTransfer>,
    pub second_flows: Vec<AssetTransfer>,
}

impl Named for DifferentialAssetFlowObserver {
    fn name(&self) -> &str {
        "DifferentialAssetFlowObserver"
    }
}

impl<S: UsesInput> Observer<S> for DifferentialAssetFlowObserver {}

impl<S: UsesInput, OTA: ObserversTuple<S>, OTB: ObserversTuple<S>>
    DifferentialObserver<OTA, OTB, S> for DifferentialAssetFlowObserver
{
}

impl<S: UsesInput, BS: Database>
    DifferentialEvmObserver<
        S,
        BS,
        (AssetFlowObserver, ()),
        (AssetFlowObserver, ()),
    > for DifferentialAssetFlowObserver
{
    fn get_first_inspector(
        &mut self,
        _pre_state: &BS,
        _input: &<S as UsesInput>::Input,
    ) -> Result<
        <(AssetFlowObserver, ()) as super::EvmObserversTuple<S, BS>>::Inspector,
        libafl::Error,
    > {
        Ok(AssetFlowInspector::new().into())
    }

    fn get_second_inspector(
        &mut self,
        _pre_state: &BS,
        _input: &<S as UsesInput>::Input,
    ) -> Result<
        <(AssetFlowObserver, ()) as super::EvmObserversTuple<S, BS>>::Inspector,
        libafl::Error,
    > {
        Ok(AssetFlowInspector::new().into())
    }

    fn on_first_executed(
        &mut self,
        _post_state: &BS,
        _inspector: InspectorTuple<BS, AssetFlowInspector, NoInspector>,
        _results: Vec<revm_primitives::ExecutionResult>,
        _input: &<S as UsesInput>::Input,
    ) -> Result<(), libafl::Error> {
        for mut transfers in _inspector.left.transfers.into_iter() {
            self.first_flows.append(&mut transfers);
        }
        Ok(())
    }

    fn on_second_executed(
        &mut self,
        _post_state: &BS,
        _inspector: InspectorTuple<BS, AssetFlowInspector, NoInspector>,
        _results: Vec<revm_primitives::ExecutionResult>,
        _input: &<S as UsesInput>::Input,
    ) -> Result<(), libafl::Error> {
        for mut transfers in _inspector.left.transfers.into_iter() {
            self.second_flows.append(&mut transfers);
        }
        Ok(())
    }
}
