file::delete("README.md");
file::rename("README-template.md", "README.md");

file::rename("_Cargo.toml", "Cargo.toml");
file::rename("{{project-name}}-entity/_Cargo.toml", "{{project-name}}-entity/Cargo.toml");
file::rename("{{project-name}}-logic/_Cargo.toml", "{{project-name}}-logic/Cargo.toml");
file::rename("{{project-name}}-migration/_Cargo.toml", "{{project-name}}-migration/Cargo.toml");
file::rename("{{project-name}}-proto/_Cargo.toml", "{{project-name}}-proto/Cargo.toml");
file::rename("{{project-name}}-server/_Cargo.toml", "{{project-name}}-server/Cargo.toml");

let project_name = variable::get("project-name");
variable::set("projectName", to_lower_camel_case(project_name));
variable::set("PROJECT_NAME", to_shouty_snake_case(project_name));
variable::set("project-name-title", to_title_case(project_name));
variable::set("project_name", to_snake_case(project_name));

let crate_name = variable::get("crate_name");
variable::set("CRATE_NAME", to_shouty_snake_case(crate_name));

let proto_ex = variable::get("proto_ex");
if proto_ex {
    variable::set("proto_ex_name", to_snake_case(variable::get("proto_ex_name")));
    variable::set("proto-ex-name", to_kebab_case(variable::get("proto_ex_name")));
    variable::set("ProtoExName", to_upper_camel_case(variable::get("proto_ex_name")));
}