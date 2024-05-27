use stats::{
    construct_update_group,
    lines::{ContractsGrowth, NewContracts},
};

construct_update_group!(Contracts {
    name: "contracts",
    charts: [NewContracts, ContractsGrowth],
});
