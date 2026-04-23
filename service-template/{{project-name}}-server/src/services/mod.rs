mod health;
{% if proto_ex %}
mod {{proto_ex_name}};
{% endif %}

pub use health::HealthService;
{% if proto_ex %}
pub use {{proto_ex_name}}::{{ProtoExName}}Impl;
{% endif %}
