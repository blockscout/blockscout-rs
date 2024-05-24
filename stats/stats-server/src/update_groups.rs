use stats::{
    construct_update_group,
    lines::{ContractsGrowth, NewContracts},
};

// todo: maybe restructure lines/counters and construct groups there

construct_update_group!(Contracts {
    name: "contracts",
    charts: [NewContracts, ContractsGrowth],
});
