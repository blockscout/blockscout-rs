file::delete("README.md");
file::rename("README-template.md", "README.md");

variable::set("projectName", to_lower_camel_case(variable::get("project-name")));
variable::set("PROJECT_NAME", to_shouty_snake_case(variable::get("project-name")));
variable::set("project-name-title", to_title_case(variable::get("project-name")));


let proto_ex = variable::get("proto_ex");
if proto_ex {
    variable::set("proto_ex_name", "TestService");
    variable::set("proto_ex_name", to_kebab_case(variable::get("proto_ex_name")));
    variable::set("ProtoExName", to_upper_camel_case(variable::get("proto_ex_name")));
    variable::set("proto_ex_name", to_pascal_case(variable::get("proto_ex_name")));
}